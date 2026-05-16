import Dispatch
import Foundation
import MultipeerConnectivity

func session(_ ptr: UnsafeMutableRawPointer) -> MCSession {
    unbox(ptr, as: MCSession.self)
}

func progress(_ ptr: UnsafeMutableRawPointer) -> Progress {
    unbox(ptr, as: Progress.self)
}

func outputStream(_ ptr: UnsafeMutableRawPointer) -> OutputStream {
    unbox(ptr, as: OutputStream.self)
}

func inputStream(_ ptr: UnsafeMutableRawPointer) -> InputStream {
    unbox(ptr, as: InputStream.self)
}

private func encryptionPreference(_ rawValue: Int32) -> MCEncryptionPreference {
    switch rawValue {
    case 1: .required
    case 2: .none
    default: .optional
    }
}

private func makeSession(
    peerPtr: UnsafeMutableRawPointer,
    identity: [AnyObject]?,
    encryptionPreference rawValue: Int32,
    errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    _ = errorOut
    let value = MCSession(
        peer: peer(peerPtr),
        securityIdentity: identity,
        encryptionPreference: encryptionPreference(rawValue)
    )
    return retainObject(value)
}

@_cdecl("mpc_session_create_with_identity")
public func mpc_session_create_with_identity(
    _ peerPtr: UnsafeMutableRawPointer,
    _ identityItems: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ identityCount: Int,
    _ encryptionPreference: Int32,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    makeSession(
        peerPtr: peerPtr,
        identity: rawObjectArray(identityItems, count: identityCount),
        encryptionPreference: encryptionPreference,
        errorOut: errorOut
    )
}

@_cdecl("mpc_session_create_with_identity_handles")
public func mpc_session_create_with_identity_handles(
    _ peerPtr: UnsafeMutableRawPointer,
    _ identityItems: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ identityCount: Int,
    _ encryptionPreference: Int32,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    makeSession(
        peerPtr: peerPtr,
        identity: boxedObjectArray(identityItems, count: identityCount),
        encryptionPreference: encryptionPreference,
        errorOut: errorOut
    )
}

@_cdecl("mpc_session_copy_my_peer")
public func mpc_session_copy_my_peer(_ sessionPtr: UnsafeMutableRawPointer) -> UnsafeMutableRawPointer? {
    retainObject(session(sessionPtr).myPeerID)
}

@_cdecl("mpc_session_copy_security_identity")
public func mpc_session_copy_security_identity(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    let identity = (session(sessionPtr).securityIdentity as? [AnyObject]) ?? []
    outCount.pointee = identity.count
    guard !identity.isEmpty else {
        outArray.pointee = nil
        return
    }
    let buffer = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(capacity: identity.count)
    for (index, item) in identity.enumerated() {
        buffer[index] = retainObject(item)
    }
    outArray.pointee = UnsafeMutableRawPointer(buffer)
}

@_cdecl("mpc_session_encryption_preference")
public func mpc_session_encryption_preference(_ sessionPtr: UnsafeMutableRawPointer) -> Int32 {
    Int32(session(sessionPtr).encryptionPreference.rawValue)
}

@_cdecl("mpc_session_copy_connected_peers")
public func mpc_session_copy_connected_peers(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outCount: UnsafeMutablePointer<Int>
) {
    writeRetainedArray(session(sessionPtr).connectedPeers, outArray: outArray, outCount: outCount)
}

@_cdecl("mpc_session_send_data")
public func mpc_session_send_data(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ dataPtr: UnsafeRawPointer?,
    _ dataLen: Int,
    _ peerPtrs: UnsafePointer<UnsafeMutableRawPointer?>?,
    _ peerCount: Int,
    _ mode: Int32,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> Int32 {
    guard dataLen >= 0 else {
        writeInvalidArgument(errorOut, "data length must not be negative")
        return MPC_INVALID_ARGUMENT
    }
    guard peerCount > 0, let peerPtrs else {
        writeInvalidArgument(errorOut, "send requires at least one destination peer")
        return MPC_INVALID_ARGUMENT
    }
    var peers: [MCPeerID] = []
    peers.reserveCapacity(peerCount)
    for index in 0 ..< peerCount {
        guard let rawPeer = peerPtrs.advanced(by: index).pointee else { continue }
        peers.append(peer(rawPeer))
    }
    guard !peers.isEmpty else {
        writeInvalidArgument(errorOut, "send requires at least one destination peer")
        return MPC_INVALID_ARGUMENT
    }
    let payload = copyRawData(dataPtr, dataLen)
    do {
        try session(sessionPtr).send(
            payload,
            toPeers: peers,
            with: mode == 1 ? .unreliable : .reliable
        )
        return MPC_OK
    } catch {
        writeNSError(errorOut, error)
        return MPC_OPERATION_FAILED
    }
}

public typealias MpcResourceSendCompletionCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?
) -> Void

@_cdecl("mpc_session_send_resource")
public func mpc_session_send_resource(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ filePath: UnsafePointer<CChar>,
    _ resourceName: UnsafePointer<CChar>,
    _ peerPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ completion: MpcResourceSendCompletionCallback?,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    let url = URL(fileURLWithPath: copyCString(filePath))
    let progress = session(sessionPtr).sendResource(
        at: url,
        withName: copyCString(resourceName),
        toPeer: peer(peerPtr)
    ) { error in
        completion?(context, retainedNSError(error))
    }
    guard let progress else {
        writeErrorOut(
            errorOut,
            BridgeErrorBox(kind: MPC_ERROR_KIND_OPERATION_FAILED, message: "sendResource returned nil")
        )
        return nil
    }
    return retainObject(progress)
}

@_cdecl("mpc_session_start_stream")
public func mpc_session_start_stream(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ streamName: UnsafePointer<CChar>,
    _ peerPtr: UnsafeMutableRawPointer,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    do {
        let stream = try session(sessionPtr).startStream(
            withName: copyCString(streamName),
            toPeer: peer(peerPtr)
        )
        return retainObject(stream as AnyObject)
    } catch {
        writeNSError(errorOut, error)
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

@_cdecl("mpc_progress_completed_unit_count")
public func mpc_progress_completed_unit_count(_ progressPtr: UnsafeMutableRawPointer) -> Int64 {
    progress(progressPtr).completedUnitCount
}

@_cdecl("mpc_progress_total_unit_count")
public func mpc_progress_total_unit_count(_ progressPtr: UnsafeMutableRawPointer) -> Int64 {
    progress(progressPtr).totalUnitCount
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
    _ length: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> Int {
    guard let bytes, length > 0 else { return 0 }
    let value = outputStream(streamPtr)
    let result = value.write(bytes.assumingMemoryBound(to: UInt8.self), maxLength: length)
    if result < 0, let error = value.streamError {
        writeNSError(errorOut, error)
    }
    return result
}

@_cdecl("mpc_input_stream_open")
public func mpc_input_stream_open(_ streamPtr: UnsafeMutableRawPointer) {
    inputStream(streamPtr).open()
}

@_cdecl("mpc_input_stream_close")
public func mpc_input_stream_close(_ streamPtr: UnsafeMutableRawPointer) {
    inputStream(streamPtr).close()
}

@_cdecl("mpc_input_stream_has_bytes_available")
public func mpc_input_stream_has_bytes_available(_ streamPtr: UnsafeMutableRawPointer) -> Bool {
    inputStream(streamPtr).hasBytesAvailable
}

@_cdecl("mpc_input_stream_read")
public func mpc_input_stream_read(
    _ streamPtr: UnsafeMutableRawPointer,
    _ bytes: UnsafeMutableRawPointer?,
    _ length: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> Int {
    guard let bytes, length > 0 else { return 0 }
    let value = inputStream(streamPtr)
    let result = value.read(bytes.assumingMemoryBound(to: UInt8.self), maxLength: length)
    if result < 0, let error = value.streamError {
        writeNSError(errorOut, error)
    }
    return result
}

@_cdecl("mpc_session_nearby_connection_data_for_peer")
public func mpc_session_nearby_connection_data_for_peer(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ peerPtr: UnsafeMutableRawPointer,
    _ outBytes: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outLen: UnsafeMutablePointer<Int>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> Int32 {
    let semaphore = DispatchSemaphore(value: 0)
    var dataResult: Data?
    var callbackError: Error?
    session(sessionPtr).nearbyConnectionData(forPeer: peer(peerPtr)) { data, error in
        dataResult = data
        callbackError = error
        semaphore.signal()
    }
    if semaphore.wait(timeout: .now() + .seconds(30)) == .timedOut {
        writeErrorOut(
            errorOut,
            BridgeErrorBox(kind: MPC_ERROR_KIND_OPERATION_FAILED, message: "timed out waiting for nearby connection data")
        )
        outBytes.pointee = nil
        outLen.pointee = 0
        return MPC_OPERATION_FAILED
    }
    if let callbackError {
        writeNSError(errorOut, callbackError)
        outBytes.pointee = nil
        outLen.pointee = 0
        return MPC_OPERATION_FAILED
    }
    guard let dataResult else {
        writeErrorOut(
            errorOut,
            BridgeErrorBox(
                kind: MPC_ERROR_KIND_OPERATION_FAILED,
                message: "nearbyConnectionDataForPeer returned no data"
            )
        )
        outBytes.pointee = nil
        outLen.pointee = 0
        return MPC_OPERATION_FAILED
    }
    outBytes.pointee = dataBuffer(dataResult)
    outLen.pointee = dataResult.count
    return MPC_OK
}

@_cdecl("mpc_session_connect_peer")
public func mpc_session_connect_peer(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ peerPtr: UnsafeMutableRawPointer,
    _ nearbyConnectionData: UnsafeRawPointer?,
    _ nearbyConnectionDataLen: Int
) {
    session(sessionPtr).connectPeer(
        peer(peerPtr),
        withNearbyConnectionData: copyRawData(nearbyConnectionData, nearbyConnectionDataLen)
    )
}

@_cdecl("mpc_session_cancel_connect_peer")
public func mpc_session_cancel_connect_peer(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ peerPtr: UnsafeMutableRawPointer
) {
    session(sessionPtr).cancelConnectPeer(peer(peerPtr))
}

@_cdecl("mpc_session_minimum_number_of_peers")
public func mpc_session_minimum_number_of_peers() -> Int {
    Int(kMCSessionMinimumNumberOfPeers)
}

@_cdecl("mpc_session_maximum_number_of_peers")
public func mpc_session_maximum_number_of_peers() -> Int {
    Int(kMCSessionMaximumNumberOfPeers)
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

public typealias MpcSessionStreamCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafePointer<CChar>,
    UnsafeMutableRawPointer?
) -> Void

public typealias MpcSessionResourceStartCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafePointer<CChar>,
    UnsafeMutableRawPointer?
) -> Void

public typealias MpcSessionResourceFinishCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafePointer<CChar>,
    UnsafePointer<CChar>?,
    UnsafeMutableRawPointer?
) -> Void

public typealias MpcSessionCertificateCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    Int
) -> Bool

private final class SessionDelegateBox: NSObject, MCSessionDelegate {
    let context: UnsafeMutableRawPointer?
    let stateCallback: MpcSessionStateCallback?
    let dataCallback: MpcSessionDataCallback?
    let streamCallback: MpcSessionStreamCallback?
    let resourceStartCallback: MpcSessionResourceStartCallback?
    let resourceFinishCallback: MpcSessionResourceFinishCallback?
    let certificateCallback: MpcSessionCertificateCallback?

    init(
        context: UnsafeMutableRawPointer?,
        stateCallback: MpcSessionStateCallback?,
        dataCallback: MpcSessionDataCallback?,
        streamCallback: MpcSessionStreamCallback?,
        resourceStartCallback: MpcSessionResourceStartCallback?,
        resourceFinishCallback: MpcSessionResourceFinishCallback?,
        certificateCallback: MpcSessionCertificateCallback?
    ) {
        self.context = context
        self.stateCallback = stateCallback
        self.dataCallback = dataCallback
        self.streamCallback = streamCallback
        self.resourceStartCallback = resourceStartCallback
        self.resourceFinishCallback = resourceFinishCallback
        self.certificateCallback = certificateCallback
    }

    func session(_ session: MCSession, peer peerID: MCPeerID, didChange state: MCSessionState) {
        stateCallback?(context, retainObject(peerID), Int32(state.rawValue))
    }

    func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        let peerPtr = retainObject(peerID)
        data.withUnsafeBytes { bytes in
            dataCallback?(context, peerPtr, bytes.baseAddress, data.count)
        }
    }

    func session(
        _ session: MCSession,
        didReceive stream: InputStream,
        withName streamName: String,
        fromPeer peerID: MCPeerID
    ) {
        streamName.withCString { name in
            streamCallback?(context, retainObject(peerID), name, retainObject(stream))
        }
    }

    func session(
        _ session: MCSession,
        didStartReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        with progress: Progress
    ) {
        resourceName.withCString { name in
            resourceStartCallback?(context, retainObject(peerID), name, retainObject(progress))
        }
    }

    func session(
        _ session: MCSession,
        didFinishReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        at localURL: URL?,
        withError error: Error?
    ) {
        resourceName.withCString { name in
            if let localURL {
                localURL.path.withCString { path in
                    resourceFinishCallback?(context, retainObject(peerID), name, path, retainedNSError(error))
                }
            } else {
                resourceFinishCallback?(context, retainObject(peerID), name, nil, retainedNSError(error))
            }
        }
    }

    func session(
        _ session: MCSession,
        didReceiveCertificate certificate: [Any]?,
        fromPeer peerID: MCPeerID,
        certificateHandler: @escaping (Bool) -> Void
    ) {
        guard let certificateCallback else {
            certificateHandler(false)
            return
        }
        let values = (certificate as? [AnyObject]) ?? []
        let buffer: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
        if values.isEmpty {
            buffer = nil
        } else {
            buffer = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(capacity: values.count)
            for (index, item) in values.enumerated() {
                buffer?[index] = retainObject(item)
            }
        }
        let accepted = certificateCallback(
            context,
            retainObject(peerID),
            buffer.map(UnsafeMutableRawPointer.init),
            values.count
        )
        certificateHandler(accepted)
    }

    override func responds(to aSelector: Selector!) -> Bool {
        if aSelector == #selector(session(_:didReceiveCertificate:fromPeer:certificateHandler:)) {
            return certificateCallback != nil
        }
        return super.responds(to: aSelector)
    }
}

private var sessionDelegates: [ObjectIdentifier: SessionDelegateBox] = [:]
private let sessionDelegatesLock = NSLock()

@_cdecl("mpc_session_set_delegate")
public func mpc_session_set_delegate(
    _ sessionPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ stateCallback: MpcSessionStateCallback?,
    _ dataCallback: MpcSessionDataCallback?,
    _ streamCallback: MpcSessionStreamCallback?,
    _ resourceStartCallback: MpcSessionResourceStartCallback?,
    _ resourceFinishCallback: MpcSessionResourceFinishCallback?,
    _ certificateCallback: MpcSessionCertificateCallback?
) {
    let value = session(sessionPtr)
    let delegate = SessionDelegateBox(
        context: context,
        stateCallback: stateCallback,
        dataCallback: dataCallback,
        streamCallback: streamCallback,
        resourceStartCallback: resourceStartCallback,
        resourceFinishCallback: resourceFinishCallback,
        certificateCallback: certificateCallback
    )
    value.delegate = delegate
    sessionDelegatesLock.lock()
    sessionDelegates[ObjectIdentifier(value)] = delegate
    sessionDelegatesLock.unlock()
}

@_cdecl("mpc_session_clear_delegate")
public func mpc_session_clear_delegate(_ sessionPtr: UnsafeMutableRawPointer) {
    let value = session(sessionPtr)
    value.delegate = nil
    sessionDelegatesLock.lock()
    sessionDelegates.removeValue(forKey: ObjectIdentifier(value))
    sessionDelegatesLock.unlock()
}
