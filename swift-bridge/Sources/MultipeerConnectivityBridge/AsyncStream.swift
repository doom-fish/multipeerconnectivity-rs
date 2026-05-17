// swiftlint:disable identifier_name file_length
import Foundation
import MultipeerConnectivity

// MARK: - Shared event callback type

public typealias MpcEventCallback = @convention(c) (
    Int32,
    UnsafeRawPointer?,
    UnsafeMutableRawPointer?
) -> Void

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
}

private final class MCSessionEventBridge: NSObject, MCSessionDelegate {
    let mcSession: MCSession
    let onEvent: MpcEventCallback
    let ctx: UnsafeMutableRawPointer?

    init(session: MCSession, onEvent: MpcEventCallback, ctx: UnsafeMutableRawPointer?) {
        self.mcSession = session
        self.onEvent = onEvent
        self.ctx = ctx
        super.init()
        session.delegate = self
    }

    deinit {
        mcSession.delegate = nil
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
        var payload = MpcSessionCertPayload(
            peerPtr: retainObject(peerID),
            itemsPtr: bufferPtr.map(UnsafeMutableRawPointer.init),
            count: values.count
        )
        withUnsafeBytes(of: &payload) { bytes in
            onEvent(5, bytes.baseAddress, ctx)
        }
        bufferPtr?.deallocate()
        // Always accept when using the async stream API.
        certificateHandler(true)
    }
}

@_cdecl("mpc_session_stream_subscribe")
public func mpc_session_stream_subscribe(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ onEvent: MpcEventCallback,
    _ ctx: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer {
    let bridge = MCSessionEventBridge(session: session(sessionPtr), onEvent: onEvent, ctx: ctx)
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("mpc_session_stream_unsubscribe")
public func mpc_session_stream_unsubscribe(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<MCSessionEventBridge>.fromOpaque(handle).release()
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

private final class MCBrowserEventBridge: NSObject, MCNearbyServiceBrowserDelegate {
    let mcBrowser: MCNearbyServiceBrowser
    let onEvent: MpcEventCallback
    let ctx: UnsafeMutableRawPointer?

    init(browser: MCNearbyServiceBrowser, onEvent: MpcEventCallback, ctx: UnsafeMutableRawPointer?) {
        self.mcBrowser = browser
        self.onEvent = onEvent
        self.ctx = ctx
        super.init()
        browser.delegate = self
    }

    deinit {
        mcBrowser.delegate = nil
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
    _ ctx: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer {
    let bridge = MCBrowserEventBridge(
        browser: browser(browserPtr),
        onEvent: onEvent,
        ctx: ctx
    )
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("mpc_browser_stream_unsubscribe")
public func mpc_browser_stream_unsubscribe(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<MCBrowserEventBridge>.fromOpaque(handle).release()
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

private final class MCAdvertiserEventBridge: NSObject, MCNearbyServiceAdvertiserDelegate {
    let mcAdvertiser: MCNearbyServiceAdvertiser
    let onEvent: MpcEventCallback
    let ctx: UnsafeMutableRawPointer?

    init(
        advertiser: MCNearbyServiceAdvertiser,
        onEvent: MpcEventCallback,
        ctx: UnsafeMutableRawPointer?
    ) {
        self.mcAdvertiser = advertiser
        self.onEvent = onEvent
        self.ctx = ctx
        super.init()
        advertiser.delegate = self
    }

    deinit {
        mcAdvertiser.delegate = nil
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
    _ ctx: UnsafeMutableRawPointer?
) -> UnsafeMutableRawPointer {
    let bridge = MCAdvertiserEventBridge(
        advertiser: advertiser(advertiserPtr),
        onEvent: onEvent,
        ctx: ctx
    )
    return Unmanaged.passRetained(bridge).toOpaque()
}

@_cdecl("mpc_advertiser_stream_unsubscribe")
public func mpc_advertiser_stream_unsubscribe(_ handle: UnsafeMutableRawPointer) {
    Unmanaged<MCAdvertiserEventBridge>.fromOpaque(handle).release()
}
