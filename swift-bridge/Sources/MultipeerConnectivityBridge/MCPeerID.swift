import Foundation
import MultipeerConnectivity

func peer(_ ptr: UnsafeMutableRawPointer) -> MCPeerID {
    unbox(ptr, as: MCPeerID.self)
}

@_cdecl("mpc_peer_id_create")
public func mpc_peer_id_create(
    _ displayName: UnsafePointer<CChar>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    let name = copyCString(displayName)
    guard !name.isEmpty else {
        writeInvalidArgument(errorOut, "display name must not be empty")
        return nil
    }
    guard name.lengthOfBytes(using: .utf8) <= 63 else {
        writeInvalidArgument(errorOut, "display name must be at most 63 UTF-8 bytes")
        return nil
    }
    return retainObject(MCPeerID(displayName: name))
}

@_cdecl("mpc_peer_id_display_name")
public func mpc_peer_id_display_name(_ peerPtr: UnsafeMutableRawPointer) -> UnsafeMutablePointer<CChar>? {
    ffiString(peer(peerPtr).displayName)
}

@_cdecl("mpc_peer_id_archive")
public func mpc_peer_id_archive(
    _ peerPtr: UnsafeMutableRawPointer,
    _ outBytes: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    _ outLen: UnsafeMutablePointer<Int>,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> Int32 {
    do {
        let data = try NSKeyedArchiver.archivedData(
            withRootObject: peer(peerPtr),
            requiringSecureCoding: true
        )
        outBytes.pointee = dataBuffer(data)
        outLen.pointee = data.count
        return MPC_OK
    } catch {
        outBytes.pointee = nil
        outLen.pointee = 0
        writeNSError(errorOut, error)
        return MPC_OPERATION_FAILED
    }
}

@_cdecl("mpc_peer_id_from_archived_data")
public func mpc_peer_id_from_archived_data(
    _ bytes: UnsafeRawPointer?,
    _ len: Int,
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> UnsafeMutableRawPointer? {
    do {
        let data = copyRawData(bytes, len)
        guard let peer = try NSKeyedUnarchiver.unarchivedObject(ofClass: MCPeerID.self, from: data) else {
            writeInvalidArgument(errorOut, "archived peer data did not decode into an MCPeerID")
            return nil
        }
        return retainObject(peer)
    } catch {
        writeNSError(errorOut, error)
        return nil
    }
}
