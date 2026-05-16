import Foundation
import MultipeerConnectivity

let MPC_OK: Int32 = 0
let MPC_INVALID_ARGUMENT: Int32 = -1
let MPC_OPERATION_FAILED: Int32 = -2

private func ffiString(_ string: String?) -> UnsafeMutablePointer<CChar>? {
    guard let string else { return nil }
    return string.withCString { strdup($0) }
}

private func writeErrorOut(
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?,
    _ message: String
) {
    errorOut?.pointee = ffiString(message)
}

private func retainObject(_ object: AnyObject) -> UnsafeMutableRawPointer {
    Unmanaged.passRetained(object).toOpaque()
}

private func makeIdentityArray(
    _ items: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ count: Int
) -> [Any]? {
    guard let items, count > 0 else { return nil }
    var array: [Any] = []
    array.reserveCapacity(count)
    for index in 0..<count {
        guard let raw = items.advanced(by: index).pointee else { continue }
        let object = Unmanaged<AnyObject>.fromOpaque(raw).takeUnretainedValue()
        array.append(object)
    }
    return array
}

private func peer(_ ptr: UnsafeMutableRawPointer) -> MCPeerID {
    Unmanaged<MCPeerID>.fromOpaque(ptr).takeUnretainedValue()
}

private func session(_ ptr: UnsafeMutableRawPointer) -> MCSession {
    Unmanaged<MCSession>.fromOpaque(ptr).takeUnretainedValue()
}

private func browser(_ ptr: UnsafeMutableRawPointer) -> MCNearbyServiceBrowser {
    Unmanaged<MCNearbyServiceBrowser>.fromOpaque(ptr).takeUnretainedValue()
}

private func advertiser(_ ptr: UnsafeMutableRawPointer) -> MCNearbyServiceAdvertiser {
    Unmanaged<MCNearbyServiceAdvertiser>.fromOpaque(ptr).takeUnretainedValue()
}

private func progress(_ ptr: UnsafeMutableRawPointer) -> Progress {
    Unmanaged<Progress>.fromOpaque(ptr).takeUnretainedValue()
}

private func outputStream(_ ptr: UnsafeMutableRawPointer) -> OutputStream {
    Unmanaged<OutputStream>.fromOpaque(ptr).takeUnretainedValue()
}

private func jsonCString(for discoveryInfo: [String: String]?) -> UnsafeMutablePointer<CChar>? {
    guard let discoveryInfo else { return nil }
    guard JSONSerialization.isValidJSONObject(discoveryInfo) else { return nil }
    guard let data = try? JSONSerialization.data(withJSONObject: discoveryInfo, options: []),
          let string = String(data: data, encoding: .utf8)
    else {
        return nil
    }
    return ffiString(string)
}

@_cdecl("mpc_string_free")
public func mpc_string_free(_ string: UnsafeMutablePointer<CChar>?) {
    guard let string else { return }
    free(string)
}

@_cdecl("mpc_object_release")
public func mpc_object_release(_ ptr: UnsafeMutableRawPointer?) {
    guard let ptr else { return }
    Unmanaged<AnyObject>.fromOpaque(ptr).release()
}

@_cdecl("mpc_object_retain")
public func mpc_object_retain(_ ptr: UnsafeMutableRawPointer?) -> UnsafeMutableRawPointer? {
    guard let ptr else { return nil }
    let object = Unmanaged<AnyObject>.fromOpaque(ptr).takeUnretainedValue()
    return retainObject(object)
}

@_cdecl("mpc_ptr_array_free")
public func mpc_ptr_array_free(_ ptr: UnsafeMutableRawPointer?) {
    guard let ptr else { return }
    ptr.assumingMemoryBound(to: UnsafeMutableRawPointer?.self).deallocate()
}

@_cdecl("mpc_peer_id_create")
public func mpc_peer_id_create(
    _ displayName: UnsafePointer<CChar>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    let name = String(cString: displayName)
    guard !name.isEmpty else {
        writeErrorOut(errorOut, "display name must not be empty")
        return nil
    }
    return retainObject(MCPeerID(displayName: name))
}

@_cdecl("mpc_peer_id_display_name")
public func mpc_peer_id_display_name(_ peerPtr: UnsafeMutableRawPointer) -> UnsafeMutablePointer<CChar>? {
    ffiString(peer(peerPtr).displayName)
}

@_cdecl("mpc_session_create_with_identity")
public func mpc_session_create_with_identity(
    _ peerPtr: UnsafeMutableRawPointer,
    _ identityItems: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ identityCount: Int,
    _ encryptionPreference: Int32,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    let preference: MCEncryptionPreference
    switch encryptionPreference {
    case 1: preference = .required
    case 2: preference = .none
    default: preference = .optional
    }
    let identity = makeIdentityArray(identityItems, identityCount)
    _ = errorOut
    let session = MCSession(
        peer: peer(peerPtr),
        securityIdentity: identity,
        encryptionPreference: preference
    )
    return retainObject(session)
}

@_cdecl("mpc_session_copy_connected_peers")
public func mpc_session_copy_connected_peers(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    let peers = session(sessionPtr).connectedPeers
    outCount.pointee = peers.count
    guard !peers.isEmpty else {
        outArray.pointee = nil
        return
    }
    let buffer = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(capacity: peers.count)
    for (index, item) in peers.enumerated() {
        buffer[index] = retainObject(item)
    }
    outArray.pointee = UnsafeMutableRawPointer(buffer)
}

@_cdecl("mpc_session_send_data")
public func mpc_session_send_data(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ dataPtr: UnsafeRawPointer?,
    _ dataLen: Int,
    _ peerPtrs: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ peerCount: Int,
    _ mode: Int32,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32 {
    guard dataLen >= 0 else {
        writeErrorOut(errorOut, "data length must not be negative")
        return MPC_INVALID_ARGUMENT
    }
    guard peerCount > 0, let peerPtrs else {
        writeErrorOut(errorOut, "send requires at least one destination peer")
        return MPC_INVALID_ARGUMENT
    }
    var peers: [MCPeerID] = []
    peers.reserveCapacity(peerCount)
    for index in 0..<peerCount {
        guard let rawPeer = peerPtrs.advanced(by: index).pointee else { continue }
        peers.append(peer(rawPeer))
    }
    guard !peers.isEmpty else {
        writeErrorOut(errorOut, "send requires at least one destination peer")
        return MPC_INVALID_ARGUMENT
    }
    let payload: Data
    if let dataPtr, dataLen > 0 {
        payload = Data(bytes: dataPtr, count: dataLen)
    } else {
        payload = Data()
    }
    do {
        try session(sessionPtr).send(
            payload,
            toPeers: peers,
            with: mode == 1 ? .unreliable : .reliable
        )
        return MPC_OK
    } catch {
        writeErrorOut(errorOut, error.localizedDescription)
        return MPC_OPERATION_FAILED
    }
}

@_cdecl("mpc_session_send_resource")
public func mpc_session_send_resource(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ filePath: UnsafePointer<CChar>,
    _ resourceName: UnsafePointer<CChar>,
    _ peerPtr: UnsafeMutableRawPointer,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    let url = URL(fileURLWithPath: String(cString: filePath))
    let progress = session(sessionPtr).sendResource(
        at: url,
        withName: String(cString: resourceName),
        toPeer: peer(peerPtr),
        withCompletionHandler: nil
    )
    guard let progress else {
        writeErrorOut(errorOut, "sendResource returned nil")
        return nil
    }
    return retainObject(progress)
}

@_cdecl("mpc_session_start_stream")
public func mpc_session_start_stream(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ streamName: UnsafePointer<CChar>,
    _ peerPtr: UnsafeMutableRawPointer,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    do {
        let stream = try session(sessionPtr).startStream(
            withName: String(cString: streamName),
            toPeer: peer(peerPtr)
        )
        return retainObject(stream)
    } catch {
        writeErrorOut(errorOut, error.localizedDescription)
        return nil
    }
}

@_cdecl("mpc_session_disconnect")
public func mpc_session_disconnect(_ sessionPtr: UnsafeMutableRawPointer) {
    session(sessionPtr).disconnect()
}

@_cdecl("mpc_progress_fraction_completed")
public func mpc_progress_fraction_completed(_ progressPtr: UnsafeMutableRawPointer) -> Double {
    progress(progressPtr).fractionCompleted
}

@_cdecl("mpc_progress_is_finished")
public func mpc_progress_is_finished(_ progressPtr: UnsafeMutableRawPointer) -> Bool {
    progress(progressPtr).isFinished
}

@_cdecl("mpc_output_stream_open")
public func mpc_output_stream_open(_ streamPtr: UnsafeMutableRawPointer) {
    outputStream(streamPtr).open()
}

@_cdecl("mpc_output_stream_close")
public func mpc_output_stream_close(_ streamPtr: UnsafeMutableRawPointer) {
    outputStream(streamPtr).close()
}

@_cdecl("mpc_output_stream_write")
public func mpc_output_stream_write(
    _ streamPtr: UnsafeMutableRawPointer,
    _ bytes: UnsafeRawPointer?,
    _ length: Int
) -> Int {
    guard let bytes, length > 0 else { return 0 }
    return outputStream(streamPtr).write(bytes.assumingMemoryBound(to: UInt8.self), maxLength: length)
}

public typealias MpcSessionStateCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    Int32
) -> Void

public typealias MpcSessionDataCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeRawPointer?,
    Int
) -> Void

private final class SessionDelegateBox: NSObject, MCSessionDelegate {
    let context: UnsafeMutableRawPointer?
    let stateCallback: MpcSessionStateCallback
    let dataCallback: MpcSessionDataCallback

    init(
        context: UnsafeMutableRawPointer?,
        stateCallback: @escaping MpcSessionStateCallback,
        dataCallback: @escaping MpcSessionDataCallback
    ) {
        self.context = context
        self.stateCallback = stateCallback
        self.dataCallback = dataCallback
    }

    func session(_ session: MCSession, peer peerID: MCPeerID, didChange state: MCSessionState) {
        stateCallback(context, retainObject(peerID), Int32(state.rawValue))
    }

    func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        let peerPtr = retainObject(peerID)
        data.withUnsafeBytes { bytes in
            dataCallback(context, peerPtr, bytes.baseAddress, data.count)
        }
    }

    func session(
        _ session: MCSession,
        didReceive stream: InputStream,
        withName streamName: String,
        fromPeer peerID: MCPeerID
    ) {}

    func session(
        _ session: MCSession,
        didStartReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        with progress: Progress
    ) {}

    func session(
        _ session: MCSession,
        didFinishReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        at localURL: URL?,
        withError error: Error?
    ) {}
}

private var sessionDelegates: [ObjectIdentifier: SessionDelegateBox] = [:]

@_cdecl("mpc_session_set_delegate")
public func mpc_session_set_delegate(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ stateCallback: @escaping MpcSessionStateCallback,
    _ dataCallback: @escaping MpcSessionDataCallback
) {
    let value = session(sessionPtr)
    let delegate = SessionDelegateBox(
        context: context,
        stateCallback: stateCallback,
        dataCallback: dataCallback
    )
    value.delegate = delegate
    sessionDelegates[ObjectIdentifier(value)] = delegate
}

@_cdecl("mpc_session_clear_delegate")
public func mpc_session_clear_delegate(_ sessionPtr: UnsafeMutableRawPointer) {
    let value = session(sessionPtr)
    value.delegate = nil
    sessionDelegates.removeValue(forKey: ObjectIdentifier(value))
}

public typealias MpcBrowserFoundCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeMutablePointer<CChar>?
) -> Void

public typealias MpcBrowserLostCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?
) -> Void

private final class BrowserDelegateBox: NSObject, MCNearbyServiceBrowserDelegate {
    let context: UnsafeMutableRawPointer?
    let foundCallback: MpcBrowserFoundCallback
    let lostCallback: MpcBrowserLostCallback

    init(
        context: UnsafeMutableRawPointer?,
        foundCallback: @escaping MpcBrowserFoundCallback,
        lostCallback: @escaping MpcBrowserLostCallback
    ) {
        self.context = context
        self.foundCallback = foundCallback
        self.lostCallback = lostCallback
    }

    func browser(
        _ browser: MCNearbyServiceBrowser,
        foundPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String : String]?
    ) {
        let peerPtr = retainObject(peerID)
        let jsonPtr = jsonCString(for: info)
        foundCallback(context, peerPtr, jsonPtr)
        if let jsonPtr {
            free(jsonPtr)
        }
    }

    func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {
        lostCallback(context, retainObject(peerID))
    }
}

private var browserDelegates: [ObjectIdentifier: BrowserDelegateBox] = [:]

@_cdecl("mpc_browser_create")
public func mpc_browser_create(
    _ peerPtr: UnsafeMutableRawPointer,
    _ serviceType: UnsafePointer<CChar>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    let type = String(cString: serviceType)
    guard !type.isEmpty else {
        writeErrorOut(errorOut, "service type must not be empty")
        return nil
    }
    return retainObject(MCNearbyServiceBrowser(peer: peer(peerPtr), serviceType: type))
}

@_cdecl("mpc_browser_start")
public func mpc_browser_start(_ browserPtr: UnsafeMutableRawPointer) {
    browser(browserPtr).startBrowsingForPeers()
}

@_cdecl("mpc_browser_stop")
public func mpc_browser_stop(_ browserPtr: UnsafeMutableRawPointer) {
    browser(browserPtr).stopBrowsingForPeers()
}

@_cdecl("mpc_browser_invite_peer")
public func mpc_browser_invite_peer(
    _ browserPtr: UnsafeMutableRawPointer,
    _ peerPtr: UnsafeMutableRawPointer,
    _ sessionPtr: UnsafeMutableRawPointer,
    _ contextBytes: UnsafeRawPointer?,
    _ contextLength: Int,
    _ timeoutSeconds: Double
) {
    let contextData: Data?
    if let contextBytes, contextLength > 0 {
        contextData = Data(bytes: contextBytes, count: contextLength)
    } else {
        contextData = nil
    }
    browser(browserPtr).invitePeer(
        peer(peerPtr),
        to: session(sessionPtr),
        withContext: contextData,
        timeout: timeoutSeconds
    )
}

@_cdecl("mpc_browser_set_delegate")
public func mpc_browser_set_delegate(
    _ browserPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ foundCallback: @escaping MpcBrowserFoundCallback,
    _ lostCallback: @escaping MpcBrowserLostCallback
) {
    let value = browser(browserPtr)
    let delegate = BrowserDelegateBox(
        context: context,
        foundCallback: foundCallback,
        lostCallback: lostCallback
    )
    value.delegate = delegate
    browserDelegates[ObjectIdentifier(value)] = delegate
}

@_cdecl("mpc_browser_clear_delegate")
public func mpc_browser_clear_delegate(_ browserPtr: UnsafeMutableRawPointer) {
    let value = browser(browserPtr)
    value.delegate = nil
    browserDelegates.removeValue(forKey: ObjectIdentifier(value))
}

public typealias MpcAdvertiserInvitationCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeRawPointer?,
    Int
) -> Bool

private final class AdvertiserDelegateBox: NSObject, MCNearbyServiceAdvertiserDelegate {
    let context: UnsafeMutableRawPointer?
    let invitationSession: MCSession?
    let invitationCallback: MpcAdvertiserInvitationCallback

    init(
        context: UnsafeMutableRawPointer?,
        invitationSession: MCSession?,
        invitationCallback: @escaping MpcAdvertiserInvitationCallback
    ) {
        self.context = context
        self.invitationSession = invitationSession
        self.invitationCallback = invitationCallback
    }

    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didReceiveInvitationFromPeer peerID: MCPeerID,
        withContext contextData: Data?,
        invitationHandler: @escaping (Bool, MCSession?) -> Void
    ) {
        let peerPtr = retainObject(peerID)
        let accepted: Bool
        if let contextData {
            accepted = contextData.withUnsafeBytes { bytes in
                invitationCallback(context, peerPtr, bytes.baseAddress, contextData.count)
            }
        } else {
            accepted = invitationCallback(context, peerPtr, nil, 0)
        }
        invitationHandler(accepted && invitationSession != nil, accepted ? invitationSession : nil)
    }
}

private var advertiserDelegates: [ObjectIdentifier: AdvertiserDelegateBox] = [:]

@_cdecl("mpc_advertiser_create")
public func mpc_advertiser_create(
    _ peerPtr: UnsafeMutableRawPointer,
    _ discoveryInfoJson: UnsafePointer<CChar>?,
    _ serviceType: UnsafePointer<CChar>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> UnsafeMutableRawPointer? {
    let info: [String: String]?
    if let discoveryInfoJson {
        let string = String(cString: discoveryInfoJson)
        if string.isEmpty {
            info = nil
        } else {
            guard let data = string.data(using: .utf8),
                  let parsed = try? JSONSerialization.jsonObject(with: data, options: []),
                  let dict = parsed as? [String: String]
            else {
                writeErrorOut(errorOut, "discoveryInfo must be a JSON object of string pairs")
                return nil
            }
            info = dict
        }
    } else {
        info = nil
    }
    let type = String(cString: serviceType)
    guard !type.isEmpty else {
        writeErrorOut(errorOut, "service type must not be empty")
        return nil
    }
    return retainObject(
        MCNearbyServiceAdvertiser(
            peer: peer(peerPtr),
            discoveryInfo: info,
            serviceType: type
        )
    )
}

@_cdecl("mpc_advertiser_start")
public func mpc_advertiser_start(_ advertiserPtr: UnsafeMutableRawPointer) {
    advertiser(advertiserPtr).startAdvertisingPeer()
}

@_cdecl("mpc_advertiser_stop")
public func mpc_advertiser_stop(_ advertiserPtr: UnsafeMutableRawPointer) {
    advertiser(advertiserPtr).stopAdvertisingPeer()
}

@_cdecl("mpc_advertiser_set_delegate")
public func mpc_advertiser_set_delegate(
    _ advertiserPtr: UnsafeMutableRawPointer,
    _ invitationSessionPtr: UnsafeMutableRawPointer?,
    _ context: UnsafeMutableRawPointer?,
    _ invitationCallback: @escaping MpcAdvertiserInvitationCallback
) {
    let value = advertiser(advertiserPtr)
    let retainedSession = invitationSessionPtr.map { session($0) }
    let delegate = AdvertiserDelegateBox(
        context: context,
        invitationSession: retainedSession,
        invitationCallback: invitationCallback
    )
    value.delegate = delegate
    advertiserDelegates[ObjectIdentifier(value)] = delegate
}

@_cdecl("mpc_advertiser_clear_delegate")
public func mpc_advertiser_clear_delegate(_ advertiserPtr: UnsafeMutableRawPointer) {
    let value = advertiser(advertiserPtr)
    value.delegate = nil
    advertiserDelegates.removeValue(forKey: ObjectIdentifier(value))
}
