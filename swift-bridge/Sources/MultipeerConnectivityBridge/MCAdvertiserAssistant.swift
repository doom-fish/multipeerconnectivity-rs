import Cocoa
import Foundation
import MultipeerConnectivity

func advertiserAssistant(_ ptr: UnsafeMutableRawPointer) -> MCAdvertiserAssistant {
    unbox(ptr, as: MCAdvertiserAssistant.self)
}

@_cdecl("mpc_advertiser_assistant_create")
public func mpc_advertiser_assistant_create(
    _ serviceType: UnsafePointer<CChar>,
    _ discoveryInfoJson: UnsafePointer<CChar>?,
    _ sessionPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    let type = copyCString(serviceType)
    guard validateServiceType(type, errorOut: nil) else { return nil }
    let info = decodeDiscoveryInfo(discoveryInfoJson, errorOut: nil)
    return onMain {
        _ = NSApplication.shared
        return retainObject(
            MCAdvertiserAssistant(
                serviceType: type,
                discoveryInfo: info,
                session: session(sessionPtr)
            )
        )
    }
}

@_cdecl("mpc_advertiser_assistant_copy_session")
public func mpc_advertiser_assistant_copy_session(
    _ assistantPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    onMain { retainObject(advertiserAssistant(assistantPtr).session) }
}

@_cdecl("mpc_advertiser_assistant_discovery_info_json")
public func mpc_advertiser_assistant_discovery_info_json(
    _ assistantPtr: UnsafeMutableRawPointer
) -> UnsafeMutablePointer<CChar>? {
    onMain { jsonCString(for: advertiserAssistant(assistantPtr).discoveryInfo) }
}

@_cdecl("mpc_advertiser_assistant_service_type")
public func mpc_advertiser_assistant_service_type(
    _ assistantPtr: UnsafeMutableRawPointer
) -> UnsafeMutablePointer<CChar>? {
    onMain { ffiString(advertiserAssistant(assistantPtr).serviceType) }
}

@_cdecl("mpc_advertiser_assistant_start")
public func mpc_advertiser_assistant_start(_ assistantPtr: UnsafeMutableRawPointer) {
    onMain {
        _ = NSApplication.shared
        advertiserAssistant(assistantPtr).start()
    }
}

@_cdecl("mpc_advertiser_assistant_stop")
public func mpc_advertiser_assistant_stop(_ assistantPtr: UnsafeMutableRawPointer) {
    onMain {
        advertiserAssistant(assistantPtr).stop()
    }
}

public typealias MpcAdvertiserAssistantCallback = @convention(c) (UnsafeMutableRawPointer?) -> Void

private final class AdvertiserAssistantDelegateBox: NSObject, MCAdvertiserAssistantDelegate {
    let context: UnsafeMutableRawPointer?
    let willPresentCallback: MpcAdvertiserAssistantCallback?
    let didDismissCallback: MpcAdvertiserAssistantCallback?

    init(
        context: UnsafeMutableRawPointer?,
        willPresentCallback: MpcAdvertiserAssistantCallback?,
        didDismissCallback: MpcAdvertiserAssistantCallback?
    ) {
        self.context = context
        self.willPresentCallback = willPresentCallback
        self.didDismissCallback = didDismissCallback
    }

    func advertiserAssistantWillPresentInvitation(_ advertiserAssistant: MCAdvertiserAssistant) {
        willPresentCallback?(context)
    }

    func advertiserAssistantDidDismissInvitation(_ advertiserAssistant: MCAdvertiserAssistant) {
        didDismissCallback?(context)
    }

    override func responds(to aSelector: Selector!) -> Bool {
        if aSelector == #selector(advertiserAssistantWillPresentInvitation(_:)) {
            return willPresentCallback != nil
        }
        if aSelector == #selector(advertiserAssistantDidDismissInvitation(_:)) {
            return didDismissCallback != nil
        }
        return super.responds(to: aSelector)
    }
}

private var advertiserAssistantDelegates: [ObjectIdentifier: AdvertiserAssistantDelegateBox] = [:]
private let advertiserAssistantDelegatesLock = NSLock()

@_cdecl("mpc_advertiser_assistant_set_delegate")
public func mpc_advertiser_assistant_set_delegate(
    _ assistantPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ willPresentCallback: MpcAdvertiserAssistantCallback?,
    _ didDismissCallback: MpcAdvertiserAssistantCallback?
) {
    onMain {
        let value = advertiserAssistant(assistantPtr)
        let delegate = AdvertiserAssistantDelegateBox(
            context: context,
            willPresentCallback: willPresentCallback,
            didDismissCallback: didDismissCallback
        )
        value.delegate = delegate
        advertiserAssistantDelegatesLock.lock()
        advertiserAssistantDelegates[ObjectIdentifier(value)] = delegate
        advertiserAssistantDelegatesLock.unlock()
    }
}

@_cdecl("mpc_advertiser_assistant_clear_delegate")
public func mpc_advertiser_assistant_clear_delegate(_ assistantPtr: UnsafeMutableRawPointer) {
    onMain {
        let value = advertiserAssistant(assistantPtr)
        value.delegate = nil
        advertiserAssistantDelegatesLock.lock()
        advertiserAssistantDelegates.removeValue(forKey: ObjectIdentifier(value))
        advertiserAssistantDelegatesLock.unlock()
    }
}
