// swiftlint:disable identifier_name file_length
import Foundation
import MultipeerConnectivity

// MARK: - Shared event callback type

public typealias MpcEventCallback = @convention(c) (
    Int32,
    UnsafeRawPointer?,
    UnsafeMutableRawPointer?
) -> Void

private let bridgeDelegateLock = NSLock()

// MARK: - MCSession async event bridge

// Event kind constants for MCSessionDelegate:
// 0 = StateChanged       payload: MpcSessionStatePayload
// 1 = DataReceived       payload: MpcSessionDataPayload
// 2 = StreamReceived     payload: MpcSessionStreamPayload
// 3 = ResourceStarted    payload: MpcSessionResourceStartPayload
// 4 = ResourceFinished   payload: MpcSessionResourceFinishPayload
// 5 = CertificateReceived  payload: MpcSessionCertPayload

private struct MpcSessionStatePayload {
    var peerPtr: UnsafeMutableRawPointer?
    var state: Int32
}

private struct MpcSessionDataPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var dataPtr: UnsafeRawPointer?
    var length: Int
}

private struct MpcSessionStreamPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var name: UnsafePointer<CChar>?
    var streamPtr: UnsafeMutableRawPointer?
}

private struct MpcSessionResourceStartPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var name: UnsafePointer<CChar>?
    var progressPtr: UnsafeMutableRawPointer?
}

private struct MpcSessionResourceFinishPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var name: UnsafePointer<CChar>?
    var urlPath: UnsafePointer<CChar>?
    var errorPtr: UnsafeMutableRawPointer?
}

private struct MpcSessionCertPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var itemsPtr: UnsafeMutableRawPointer?
    var count: Int
    var handlerPtr: UnsafeMutableRawPointer?
}

final class MpcCertificateHandlerBox: NSObject {
    private var handler: ((Bool) -> Void)?

    init(handler: @escaping (Bool) -> Void) {
        self.handler = handler
    }

    func invoke(_ accept: Bool) {
        handler?(accept)
        handler = nil
    }
}

@_cdecl("mpc_certificate_handle_respond")
public func mpc_certificate_handle_respond(_ handlePtr: UnsafeMutableRawPointer, _ accept: Bool) {
    let box = Unmanaged<MpcCertificateHandlerBox>.fromOpaque(handlePtr).takeRetainedValue()
    box.invoke(accept)
}

private final class MCSessionEventBridge: NSObject, MCSessionDelegate, MpcDelegateIdentity {
    let mcSession: MCSession
    let onEvent: MpcEventCallback
    let ctx: UnsafeMutableRawPointer?
    let retention: ContextRetention
    private weak var previous: (any MCSessionDelegate)?
    private(set) var isDetached = false

    init(
        session: MCSession,
        onEvent: MpcEventCallback,
        ctx: UnsafeMutableRawPointer?,
        ctxRetain: MpcContextRetainCallback,
        ctxRelease: MpcContextRetainCallback
    ) {
        self.mcSession = session
        self.onEvent = onEvent
        self.ctx = ctx
        self.retention = ContextRetention(context: ctx, retain: ctxRetain, release: ctxRelease)
        super.init()
        bridgeDelegateLock.lock()
        previous = session.delegate
        session.delegate = self
        bridgeDelegateLock.unlock()
    }

    var delegateIdentity: UnsafeMutableRawPointer? {
        Unmanaged.passUnretained(self).toOpaque()
    }

    func detach() {
        bridgeDelegateLock.lock()
        defer { bridgeDelegateLock.unlock() }
        isDetached = true
        guard mcSession.delegate === self else { return }
        if let previous = previous as? MCSessionEventBridge, !previous.isDetached {
            mcSession.delegate = previous
        } else {
            mcSession.delegate = registeredSessionDelegate(for: mcSession)
        }
    }

    func session(_ session: MCSession, peer peerID: MCPeerID, didChange state: MCSessionState) {
        var payload = MpcSessionStatePayload(
            peerPtr: retainObject(peerID),
            state: Int32(state.rawValue)
        )
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(0, bytes.baseAddress, ctx)
        }
    }

    func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        data.withUnsafeBytes { dataBytes in
            var payload = MpcSessionDataPayload(
                peerPtr: retainObject(peerID),
                dataPtr: dataBytes.baseAddress,
                length: data.count
            )
            withUnsafeBytes(of: &payload) { bytes in
                onEvent(1, bytes.baseAddress, ctx)
            }
        }
    }

    func session(
        _ session: MCSession,
        didReceive stream: InputStream,
        withName streamName: String,
        fromPeer peerID: MCPeerID
    ) {
        streamName.withCString { nameCStr in
            var payload = MpcSessionStreamPayload(
                peerPtr: retainObject(peerID),
                name: nameCStr,
                streamPtr: retainObject(stream)
            )
            withUnsafeBytes(of: &payload) { bytes in
                onEvent(2, bytes.baseAddress, ctx)
            }
        }
    }

    func session(
        _ session: MCSession,
        didStartReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        with progress: Progress
    ) {
        resourceName.withCString { nameCStr in
            var payload = MpcSessionResourceStartPayload(
                peerPtr: retainObject(peerID),
                name: nameCStr,
                progressPtr: retainObject(progress)
            )
            withUnsafeBytes(of: &payload) { bytes in
                onEvent(3, bytes.baseAddress, ctx)
            }
        }
    }

    func session(
        _ session: MCSession,
        didFinishReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        at localURL: URL?,
        withError error: Error?
    ) {
        let errPtr = retainedNSError(error)
        resourceName.withCString { nameCStr in
            let doCallback = { (pathPtr: UnsafePointer<CChar>?) in
                var payload = MpcSessionResourceFinishPayload(
                    peerPtr: retainObject(peerID),
                    name: nameCStr,
                    urlPath: pathPtr,
                    errorPtr: errPtr
                )
                withUnsafeBytes(of: &payload) { bytes in
                    self.onEvent(4, bytes.baseAddress, self.ctx)
                }
            }
            if let urlPath = localURL?.path {
                urlPath.withCString(doCallback)
            } else {
                doCallback(nil)
            }
        }
    }

    func session(
        _ session: MCSession,
        didReceiveCertificate certificate: [Any]?,
        fromPeer peerID: MCPeerID,
        certificateHandler: @escaping (Bool) -> Void
    ) {
        let values = (certificate as? [AnyObject]) ?? []
        let bufferPtr: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
        if values.isEmpty {
            bufferPtr = nil
        } else {
            bufferPtr = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(capacity: values.count)
            for (index, item) in values.enumerated() {
                bufferPtr?[index] = retainObject(item)
            }
        }
        let handlerBox = MpcCertificateHandlerBox(handler: certificateHandler)
        var payload = MpcSessionCertPayload(
            peerPtr: retainObject(peerID),
            itemsPtr: bufferPtr.map(UnsafeMutableRawPointer.init),
            count: values.count,
            handlerPtr: Unmanaged.passRetained(handlerBox).toOpaque()
        )
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(5, bytes.baseAddress, ctx)
        }
        bufferPtr?.deallocate()
    }
}

@_cdecl("mpc_session_stream_subscribe")
public func mpc_session_stream_subscribe(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ onEvent: MpcEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ ctxRetain: MpcContextRetainCallback,
    _ ctxRelease: MpcContextRetainCallback
) -> UnsafeMutableRawPointer {
    let bridge = MCSessionEventBridge(
        session: session(sessionPtr),
        onEvent: onEvent,
        ctx: ctx,
        ctxRetain: ctxRetain,
        ctxRelease: ctxRelease
    )
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("mpc_session_stream_unsubscribe")
public func mpc_session_stream_unsubscribe(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<MCSessionEventBridge>.fromOpaque(handle).takeRetainedValue().detach()
}

public typealias MpcCertificateDecisionCallback = @convention(c) (UnsafeMutableRawPointer?, Bool) -> Void

@_cdecl("mpc_session_stream_deliver_certificate")
public func mpc_session_stream_deliver_certificate(
    _ handle: UnsafeMutableRawPointer,
    _ peerPtr: UnsafeMutableRawPointer,
    _ includeItem: Bool,
    _ decision: MpcCertificateDecisionCallback,
    _ decisionContext: UnsafeMutableRawPointer?
) {
    let bridge = Unmanaged<MCSessionEventBridge>.fromOpaque(handle).takeUnretainedValue()
    let certificate: [Any]? = includeItem ? [NSObject()] : nil
    bridge.session(
        bridge.mcSession,
        didReceiveCertificate: certificate,
        fromPeer: peer(peerPtr)
    ) { accepted in
        decision(decisionContext, accepted)
    }
}

// MARK: - MCNearbyServiceBrowser async event bridge

// Event kind constants for MCNearbyServiceBrowserDelegate:
// 0 = FoundPeer       payload: MpcBrowserFoundPayload
// 1 = LostPeer        payload: MpcBrowserLostPayload
// 2 = BrowsingError   payload: MpcBrowserErrorPayload

private struct MpcBrowserFoundPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var discoveryJson: UnsafeMutablePointer<CChar>?
}

private struct MpcBrowserLostPayload {
    var peerPtr: UnsafeMutableRawPointer?
}

private struct MpcBrowserErrorPayload {
    var errorPtr: UnsafeMutableRawPointer?
}

private final class MCBrowserEventBridge: NSObject, MCNearbyServiceBrowserDelegate, MpcDelegateIdentity {
    let mcBrowser: MCNearbyServiceBrowser
    let onEvent: MpcEventCallback
    let ctx: UnsafeMutableRawPointer?
    let retention: ContextRetention
    private weak var previous: (any MCNearbyServiceBrowserDelegate)?
    private(set) var isDetached = false

    init(
        browser: MCNearbyServiceBrowser,
        onEvent: MpcEventCallback,
        ctx: UnsafeMutableRawPointer?,
        ctxRetain: MpcContextRetainCallback,
        ctxRelease: MpcContextRetainCallback
    ) {
        self.mcBrowser = browser
        self.onEvent = onEvent
        self.ctx = ctx
        self.retention = ContextRetention(context: ctx, retain: ctxRetain, release: ctxRelease)
        super.init()
        bridgeDelegateLock.lock()
        previous = browser.delegate
        browser.delegate = self
        bridgeDelegateLock.unlock()
    }

    var delegateIdentity: UnsafeMutableRawPointer? {
        Unmanaged.passUnretained(self).toOpaque()
    }

    func detach() {
        bridgeDelegateLock.lock()
        defer { bridgeDelegateLock.unlock() }
        isDetached = true
        guard mcBrowser.delegate === self else { return }
        if let previous = previous as? MCBrowserEventBridge, !previous.isDetached {
            mcBrowser.delegate = previous
        } else {
            mcBrowser.delegate = registeredBrowserDelegate(for: mcBrowser)
        }
    }

    func browser(
        _ browser: MCNearbyServiceBrowser,
        foundPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String: String]?
    ) {
        let jsonPtr = jsonCString(for: info)
        var payload = MpcBrowserFoundPayload(
            peerPtr: retainObject(peerID),
            discoveryJson: jsonPtr
        )
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(0, bytes.baseAddress, ctx)
        }
        if let jsonPtr { free(jsonPtr) }
    }

    func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {
        var payload = MpcBrowserLostPayload(peerPtr: retainObject(peerID))
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(1, bytes.baseAddress, ctx)
        }
    }

    func browser(_ browser: MCNearbyServiceBrowser, didNotStartBrowsingForPeers error: any Error) {
        var payload = MpcBrowserErrorPayload(errorPtr: retainedNSError(error))
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(2, bytes.baseAddress, ctx)
        }
    }
}

@_cdecl("mpc_browser_stream_subscribe")
public func mpc_browser_stream_subscribe(
    _ browserPtr: UnsafeMutableRawPointer,
    _ onEvent: MpcEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ ctxRetain: MpcContextRetainCallback,
    _ ctxRelease: MpcContextRetainCallback
) -> UnsafeMutableRawPointer {
    let bridge = MCBrowserEventBridge(
        browser: browser(browserPtr),
        onEvent: onEvent,
        ctx: ctx,
        ctxRetain: ctxRetain,
        ctxRelease: ctxRelease
    )
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("mpc_browser_stream_unsubscribe")
public func mpc_browser_stream_unsubscribe(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<MCBrowserEventBridge>.fromOpaque(handle).takeRetainedValue().detach()
}

// MARK: - MCNearbyServiceAdvertiser async event bridge

// Event kind constants for MCNearbyServiceAdvertiserDelegate:
// 0 = ReceivedInvitation   payload: MpcAdvertiserInvitationPayload
// 1 = AdvertisingError     payload: MpcAdvertiserErrorPayload

private struct MpcAdvertiserInvitationPayload {
    var peerPtr: UnsafeMutableRawPointer?
    var contextPtr: UnsafeRawPointer?
    var contextLen: Int
    var invitationHandlePtr: UnsafeMutableRawPointer?
}

private struct MpcAdvertiserErrorPayload {
    var errorPtr: UnsafeMutableRawPointer?
}

// Holds the Swift invitationHandler closure until Rust calls accept/decline.
final class MpcInvitationHandlerBox: NSObject {
    private var handler: ((Bool, MCSession?) -> Void)?

    init(handler: @escaping (Bool, MCSession?) -> Void) {
        self.handler = handler
    }

    func invoke(_ accept: Bool, mcSession: MCSession?) {
        handler?(accept, mcSession)
        handler = nil
    }
}

@_cdecl("mpc_invitation_handle_accept")
public func mpc_invitation_handle_accept(
    _ handlePtr: UnsafeMutableRawPointer,
    _ sessionPtr: UnsafeMutableRawPointer
) {
    let box = Unmanaged<MpcInvitationHandlerBox>.fromOpaque(handlePtr).takeRetainedValue()
    box.invoke(true, mcSession: session(sessionPtr))
}

@_cdecl("mpc_invitation_handle_decline")
public func mpc_invitation_handle_decline(_ handlePtr: UnsafeMutableRawPointer) {
    let box = Unmanaged<MpcInvitationHandlerBox>.fromOpaque(handlePtr).takeRetainedValue()
    box.invoke(false, mcSession: nil)
}

private final class MCAdvertiserEventBridge: NSObject, MCNearbyServiceAdvertiserDelegate, MpcDelegateIdentity {
    let mcAdvertiser: MCNearbyServiceAdvertiser
    let onEvent: MpcEventCallback
    let ctx: UnsafeMutableRawPointer?
    let retention: ContextRetention
    private weak var previous: (any MCNearbyServiceAdvertiserDelegate)?
    private(set) var isDetached = false

    init(
        advertiser: MCNearbyServiceAdvertiser,
        onEvent: MpcEventCallback,
        ctx: UnsafeMutableRawPointer?,
        ctxRetain: MpcContextRetainCallback,
        ctxRelease: MpcContextRetainCallback
    ) {
        self.mcAdvertiser = advertiser
        self.onEvent = onEvent
        self.ctx = ctx
        self.retention = ContextRetention(context: ctx, retain: ctxRetain, release: ctxRelease)
        super.init()
        bridgeDelegateLock.lock()
        previous = advertiser.delegate
        advertiser.delegate = self
        bridgeDelegateLock.unlock()
    }

    var delegateIdentity: UnsafeMutableRawPointer? {
        Unmanaged.passUnretained(self).toOpaque()
    }

    func detach() {
        bridgeDelegateLock.lock()
        defer { bridgeDelegateLock.unlock() }
        isDetached = true
        guard mcAdvertiser.delegate === self else { return }
        if let previous = previous as? MCAdvertiserEventBridge, !previous.isDetached {
            mcAdvertiser.delegate = previous
        } else {
            mcAdvertiser.delegate = registeredAdvertiserDelegate(for: mcAdvertiser)
        }
    }

    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didReceiveInvitationFromPeer peerID: MCPeerID,
        withContext contextData: Data?,
        invitationHandler: @escaping (Bool, MCSession?) -> Void
    ) {
        let handlerBox = MpcInvitationHandlerBox(handler: invitationHandler)
        let handlerPtr = Unmanaged.passRetained(handlerBox).toOpaque()
        let doCallback = { (ctxPtr: UnsafeRawPointer?, ctxLen: Int) in
            var payload = MpcAdvertiserInvitationPayload(
                peerPtr: retainObject(peerID),
                contextPtr: ctxPtr,
                contextLen: ctxLen,
                invitationHandlePtr: handlerPtr
            )
            withUnsafeBytes(of: &payload) { bytes in
                self.onEvent(0, bytes.baseAddress, self.ctx)
            }
        }
        if let contextData {
            contextData.withUnsafeBytes { bytes in
                doCallback(bytes.baseAddress, contextData.count)
            }
        } else {
            doCallback(nil, 0)
        }
    }

    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didNotStartAdvertisingPeer error: any Error
    ) {
        var payload = MpcAdvertiserErrorPayload(errorPtr: retainedNSError(error))
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(1, bytes.baseAddress, ctx)
        }
    }
}

@_cdecl("mpc_advertiser_stream_subscribe")
public func mpc_advertiser_stream_subscribe(
    _ advertiserPtr: UnsafeMutableRawPointer,
    _ onEvent: MpcEventCallback,
    _ ctx: UnsafeMutableRawPointer?,
    _ ctxRetain: MpcContextRetainCallback,
    _ ctxRelease: MpcContextRetainCallback
) -> UnsafeMutableRawPointer {
    let bridge = MCAdvertiserEventBridge(
        advertiser: advertiser(advertiserPtr),
        onEvent: onEvent,
        ctx: ctx,
        ctxRetain: ctxRetain,
        ctxRelease: ctxRelease
    )
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("mpc_advertiser_stream_unsubscribe")
public func mpc_advertiser_stream_unsubscribe(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<MCAdvertiserEventBridge>.fromOpaque(handle).takeRetainedValue().detach()
}

// MARK: - FFI Layout Verification

/// Cross-language ABI check called from Rust's `tests/async_stream_tests.rs`.
///
/// Returns `true` only if the Swift `MemoryLayout` (stride and alignment) of
/// every packed event payload struct matches the values pinned on the Rust side
/// via the `const _: () = assert!(...)` checks in `src/async_api.rs`. A `false`
/// return flags a genuine Rust/Swift ABI mismatch.
@_cdecl("mpc_async_verify_ffi_layout")
public func mpc_async_verify_ffi_layout() -> Bool {
    MemoryLayout<MpcSessionStatePayload>.stride == 16
        && MemoryLayout<MpcSessionStatePayload>.alignment == 8
        && MemoryLayout<MpcSessionDataPayload>.stride == 24
        && MemoryLayout<MpcSessionDataPayload>.alignment == 8
        && MemoryLayout<MpcSessionStreamPayload>.stride == 24
        && MemoryLayout<MpcSessionStreamPayload>.alignment == 8
        && MemoryLayout<MpcSessionResourceStartPayload>.stride == 24
        && MemoryLayout<MpcSessionResourceStartPayload>.alignment == 8
        && MemoryLayout<MpcSessionResourceFinishPayload>.stride == 32
        && MemoryLayout<MpcSessionResourceFinishPayload>.alignment == 8
        && MemoryLayout<MpcSessionCertPayload>.stride == 32
        && MemoryLayout<MpcSessionCertPayload>.alignment == 8
        && MemoryLayout<MpcBrowserFoundPayload>.stride == 16
        && MemoryLayout<MpcBrowserFoundPayload>.alignment == 8
        && MemoryLayout<MpcBrowserLostPayload>.stride == 8
        && MemoryLayout<MpcBrowserLostPayload>.alignment == 8
        && MemoryLayout<MpcBrowserErrorPayload>.stride == 8
        && MemoryLayout<MpcBrowserErrorPayload>.alignment == 8
        && MemoryLayout<MpcAdvertiserInvitationPayload>.stride == 32
        && MemoryLayout<MpcAdvertiserInvitationPayload>.alignment == 8
        && MemoryLayout<MpcAdvertiserErrorPayload>.stride == 8
        && MemoryLayout<MpcAdvertiserErrorPayload>.alignment == 8
}
