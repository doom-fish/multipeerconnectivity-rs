import Cocoa
import Foundation
import MultipeerConnectivity

func browserViewController(_ ptr: UnsafeMutableRawPointer) -> MCBrowserViewController {
    unbox(ptr, as: MCBrowserViewController.self)
}

@_cdecl("mpc_browser_view_controller_create_with_service_type")
public func mpc_browser_view_controller_create_with_service_type(
    _ serviceType: UnsafePointer<CChar>,
    _ sessionPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    let type = copyCString(serviceType)
    guard validateServiceType(type, errorOut: nil) else { return nil }
    return onMain {
        _ = NSApplication.shared
        return retainObject(MCBrowserViewController(serviceType: type, session: session(sessionPtr)))
    }
}

@_cdecl("mpc_browser_view_controller_create_with_browser")
public func mpc_browser_view_controller_create_with_browser(
    _ browserPtr: UnsafeMutableRawPointer,
    _ sessionPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    onMain {
        _ = NSApplication.shared
        return retainObject(MCBrowserViewController(browser: browser(browserPtr), session: session(sessionPtr)))
    }
}

@_cdecl("mpc_browser_view_controller_copy_browser")
public func mpc_browser_view_controller_copy_browser(
    _ controllerPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    onMain { retainObject(browserViewController(controllerPtr).browser) }
}

@_cdecl("mpc_browser_view_controller_copy_session")
public func mpc_browser_view_controller_copy_session(
    _ controllerPtr: UnsafeMutableRawPointer
) -> UnsafeMutableRawPointer? {
    onMain { retainObject(browserViewController(controllerPtr).session) }
}

@_cdecl("mpc_browser_view_controller_minimum_number_of_peers")
public func mpc_browser_view_controller_minimum_number_of_peers(
    _ controllerPtr: UnsafeMutableRawPointer
) -> Int {
    onMain { browserViewController(controllerPtr).minimumNumberOfPeers }
}

@_cdecl("mpc_browser_view_controller_set_minimum_number_of_peers")
public func mpc_browser_view_controller_set_minimum_number_of_peers(
    _ controllerPtr: UnsafeMutableRawPointer,
    _ value: Int
) {
    onMain {
        browserViewController(controllerPtr).minimumNumberOfPeers = value
    }
}

@_cdecl("mpc_browser_view_controller_maximum_number_of_peers")
public func mpc_browser_view_controller_maximum_number_of_peers(
    _ controllerPtr: UnsafeMutableRawPointer
) -> Int {
    onMain { browserViewController(controllerPtr).maximumNumberOfPeers }
}

@_cdecl("mpc_browser_view_controller_set_maximum_number_of_peers")
public func mpc_browser_view_controller_set_maximum_number_of_peers(
    _ controllerPtr: UnsafeMutableRawPointer,
    _ value: Int
) {
    onMain {
        browserViewController(controllerPtr).maximumNumberOfPeers = value
    }
}

public typealias MpcBrowserViewControllerCallback = @convention(c) (UnsafeMutableRawPointer?) -> Void
public typealias MpcBrowserViewControllerShouldPresentCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?,
    UnsafeMutablePointer<CChar>?
) -> Bool

private final class BrowserViewControllerDelegateBox: NSObject, MCBrowserViewControllerDelegate {
    let context: UnsafeMutableRawPointer?
    let finishCallback: MpcBrowserViewControllerCallback?
    let cancelCallback: MpcBrowserViewControllerCallback?
    let shouldPresentCallback: MpcBrowserViewControllerShouldPresentCallback?

    init(
        context: UnsafeMutableRawPointer?,
        finishCallback: MpcBrowserViewControllerCallback?,
        cancelCallback: MpcBrowserViewControllerCallback?,
        shouldPresentCallback: MpcBrowserViewControllerShouldPresentCallback?
    ) {
        self.context = context
        self.finishCallback = finishCallback
        self.cancelCallback = cancelCallback
        self.shouldPresentCallback = shouldPresentCallback
    }

    func browserViewControllerDidFinish(_ browserViewController: MCBrowserViewController) {
        finishCallback?(context)
    }

    func browserViewControllerWasCancelled(_ browserViewController: MCBrowserViewController) {
        cancelCallback?(context)
    }

    func browserViewController(
        _ browserViewController: MCBrowserViewController,
        shouldPresentNearbyPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String: String]?
    ) -> Bool {
        let jsonPtr = jsonCString(for: info)
        defer {
            if let jsonPtr {
                free(jsonPtr)
            }
        }
        return shouldPresentCallback?(context, retainObject(peerID), jsonPtr) ?? true
    }

    override func responds(to aSelector: Selector!) -> Bool {
        if aSelector == #selector(browserViewController(_:shouldPresentNearbyPeer:withDiscoveryInfo:)) {
            return shouldPresentCallback != nil
        }
        return super.responds(to: aSelector)
    }
}

private var browserViewControllerDelegates: [ObjectIdentifier: BrowserViewControllerDelegateBox] = [:]
private let browserViewControllerDelegatesLock = NSLock()

@_cdecl("mpc_browser_view_controller_set_delegate")
public func mpc_browser_view_controller_set_delegate(
    _ controllerPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ finishCallback: MpcBrowserViewControllerCallback?,
    _ cancelCallback: MpcBrowserViewControllerCallback?,
    _ shouldPresentCallback: MpcBrowserViewControllerShouldPresentCallback?
) {
    onMain {
        let value = browserViewController(controllerPtr)
        let delegate = BrowserViewControllerDelegateBox(
            context: context,
            finishCallback: finishCallback,
            cancelCallback: cancelCallback,
            shouldPresentCallback: shouldPresentCallback
        )
        value.delegate = delegate
        browserViewControllerDelegatesLock.lock()
        browserViewControllerDelegates[ObjectIdentifier(value)] = delegate
        browserViewControllerDelegatesLock.unlock()
    }
}

@_cdecl("mpc_browser_view_controller_clear_delegate")
public func mpc_browser_view_controller_clear_delegate(_ controllerPtr: UnsafeMutableRawPointer) {
    onMain {
        let value = browserViewController(controllerPtr)
        value.delegate = nil
        browserViewControllerDelegatesLock.lock()
        browserViewControllerDelegates.removeValue(forKey: ObjectIdentifier(value))
        browserViewControllerDelegatesLock.unlock()
    }
}
