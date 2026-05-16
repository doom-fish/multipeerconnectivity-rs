import Foundation
import MultipeerConnectivity

func advertiser(_ ptr: UnsafeMutableRawPointer) -> MCNearbyServiceAdvertiser {
    unbox(ptr, as: MCNearbyServiceAdvertiser.self)
}

@_cdecl("mpc_advertiser_create")
public func mpc_advertiser_create(
    _ peerPtr: UnsafeMutableRawPointer,
    _ discoveryInfoJson: UnsafePointer<CChar>?,
    _ serviceType: UnsafePointer<CChar>
) -> UnsafeMutableRawPointer? {
    let type = copyCString(serviceType)
    guard validateServiceType(type, errorOut: nil) else { return nil }
    let info = decodeDiscoveryInfo(discoveryInfoJson, errorOut: nil)
    return retainObject(
        MCNearbyServiceAdvertiser(
            peer: peer(peerPtr),
            discoveryInfo: info,
            serviceType: type
        )
    )
}

@_cdecl("mpc_advertiser_copy_my_peer")
public func mpc_advertiser_copy_my_peer(
    _ advertiserPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    retainObject(advertiser(advertiserPtr).myPeerID)
}

@_cdecl("mpc_advertiser_discovery_info_json")
public func mpc_advertiser_discovery_info_json(
    _ advertiserPtr: UnsafeMutableRawPointer
) -> UnsafeMutablePointer<CChar>? {
    jsonCString(for: advertiser(advertiserPtr).discoveryInfo)
}

@_cdecl("mpc_advertiser_service_type")
public func mpc_advertiser_service_type(_ advertiserPtr: UnsafeMutableRawPointer) -> UnsafeMutablePointer<CChar>? {
    ffiString(advertiser(advertiserPtr).serviceType)
}

@_cdecl("mpc_advertiser_start")
public func mpc_advertiser_start(_ advertiserPtr: UnsafeMutableRawPointer) {
    advertiser(advertiserPtr).startAdvertisingPeer()
}

@_cdecl("mpc_advertiser_stop")
public func mpc_advertiser_stop(_ advertiserPtr: UnsafeMutableRawPointer) {
    advertiser(advertiserPtr).stopAdvertisingPeer()
}

public typealias MpcAdvertiserInvitationCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeRawPointer?,
    Int
) -> UnsafeMutableRawPointer?

public typealias MpcAdvertiserErrorCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?
) -> Void

private final class AdvertiserDelegateBox: NSObject, MCNearbyServiceAdvertiserDelegate {
    let context: UnsafeMutableRawPointer?
    let invitationCallback: MpcAdvertiserInvitationCallback?
    let errorCallback: MpcAdvertiserErrorCallback?

    init(
        context: UnsafeMutableRawPointer?,
        invitationCallback: MpcAdvertiserInvitationCallback?,
        errorCallback: MpcAdvertiserErrorCallback?
    ) {
        self.context = context
        self.invitationCallback = invitationCallback
        self.errorCallback = errorCallback
    }

    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didReceiveInvitationFromPeer peerID: MCPeerID,
        withContext contextData: Data?,
        invitationHandler: @escaping (Bool, MCSession?) -> Void
    ) {
        let peerPtr = retainObject(peerID)
        let sessionPtr: UnsafeMutableRawPointer?
        if let contextData {
            sessionPtr = contextData.withUnsafeBytes { bytes in
                invitationCallback?(context, peerPtr, bytes.baseAddress, contextData.count)
            }
        } else {
            sessionPtr = invitationCallback?(context, peerPtr, nil, 0)
        }
        guard let sessionPtr else {
            invitationHandler(false, nil)
            return
        }
        let invitedSession = session(sessionPtr)
        invitationHandler(true, invitedSession)
        mpc_object_release(sessionPtr)
    }

    func advertiser(_ advertiser: MCNearbyServiceAdvertiser, didNotStartAdvertisingPeer error: any Error) {
        errorCallback?(context, retainedNSError(error))
    }

    override func responds(to aSelector: Selector!) -> Bool {
        if aSelector == #selector(advertiser(_:didNotStartAdvertisingPeer:)) {
            return errorCallback != nil
        }
        return super.responds(to: aSelector)
    }
}

private var advertiserDelegates: [ObjectIdentifier: AdvertiserDelegateBox] = [:]
private let advertiserDelegatesLock = NSLock()

@_cdecl("mpc_advertiser_set_delegate")
public func mpc_advertiser_set_delegate(
    _ advertiserPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ invitationCallback: MpcAdvertiserInvitationCallback?,
    _ errorCallback: MpcAdvertiserErrorCallback?
) {
    let value = advertiser(advertiserPtr)
    let delegate = AdvertiserDelegateBox(
        context: context,
        invitationCallback: invitationCallback,
        errorCallback: errorCallback
    )
    value.delegate = delegate
    advertiserDelegatesLock.lock()
    advertiserDelegates[ObjectIdentifier(value)] = delegate
    advertiserDelegatesLock.unlock()
}

@_cdecl("mpc_advertiser_clear_delegate")
public func mpc_advertiser_clear_delegate(_ advertiserPtr: UnsafeMutableRawPointer) {
    let value = advertiser(advertiserPtr)
    value.delegate = nil
    advertiserDelegatesLock.lock()
    advertiserDelegates.removeValue(forKey: ObjectIdentifier(value))
    advertiserDelegatesLock.unlock()
}
