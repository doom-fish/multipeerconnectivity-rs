import Foundation
import MultipeerConnectivity

func browser(_ ptr: UnsafeMutableRawPointer) -> MCNearbyServiceBrowser {
    unbox(ptr, as: MCNearbyServiceBrowser.self)
}

@_cdecl("mpc_browser_create")
public func mpc_browser_create(
    _ peerPtr: UnsafeMutableRawPointer,
    _ serviceType: UnsafePointer<CChar>
) -> UnsafeMutableRawPointer? {
    let type = copyCString(serviceType)
    guard validateServiceType(type, errorOut: nil) else { return nil }
    return retainObject(MCNearbyServiceBrowser(peer: peer(peerPtr), serviceType: type))
}

@_cdecl("mpc_browser_copy_my_peer")
public func mpc_browser_copy_my_peer(_ browserPtr: UnsafeMutableRawPointer) -> UnsafeMutableRawPointer? {
    retainObject(browser(browserPtr).myPeerID)
}

@_cdecl("mpc_browser_service_type")
public func mpc_browser_service_type(_ browserPtr: UnsafeMutableRawPointer) -> UnsafeMutablePointer<CChar>? {
    ffiString(browser(browserPtr).serviceType)
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
    browser(browserPtr).invitePeer(
        peer(peerPtr),
        to: session(sessionPtr),
        withContext: contextLength > 0 ? copyRawData(contextBytes, contextLength) : nil,
        timeout: timeoutSeconds
    )
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

public typealias MpcBrowserErrorCallback = @convention(c) (
    UnsafeMutableRawPointer?,
    UnsafeMutableRawPointer?
) -> Void

private final class BrowserDelegateBox: NSObject, MCNearbyServiceBrowserDelegate {
    let context: UnsafeMutableRawPointer?
    let foundCallback: MpcBrowserFoundCallback?
    let lostCallback: MpcBrowserLostCallback?
    let errorCallback: MpcBrowserErrorCallback?

    init(
        context: UnsafeMutableRawPointer?,
        foundCallback: MpcBrowserFoundCallback?,
        lostCallback: MpcBrowserLostCallback?,
        errorCallback: MpcBrowserErrorCallback?
    ) {
        self.context = context
        self.foundCallback = foundCallback
        self.lostCallback = lostCallback
        self.errorCallback = errorCallback
    }

    func browser(
        _ browser: MCNearbyServiceBrowser,
        foundPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String: String]?
    ) {
        let peerPtr = retainObject(peerID)
        let jsonPtr = jsonCString(for: info)
        foundCallback?(context, peerPtr, jsonPtr)
        if let jsonPtr {
            free(jsonPtr)
        }
    }

    func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {
        lostCallback?(context, retainObject(peerID))
    }

    func browser(_ browser: MCNearbyServiceBrowser, didNotStartBrowsingForPeers error: any Error) {
        errorCallback?(context, retainedNSError(error))
    }

    override func responds(to aSelector: Selector!) -> Bool {
        if aSelector == #selector(browser(_:didNotStartBrowsingForPeers:)) {
            return errorCallback != nil
        }
        return super.responds(to: aSelector)
    }
}

private var browserDelegates: [ObjectIdentifier: BrowserDelegateBox] = [:]
private let browserDelegatesLock = NSLock()

@_cdecl("mpc_browser_set_delegate")
public func mpc_browser_set_delegate(
    _ browserPtr: UnsafeMutableRawPointer,
    _ context: UnsafeMutableRawPointer?,
    _ foundCallback: MpcBrowserFoundCallback?,
    _ lostCallback: MpcBrowserLostCallback?,
    _ errorCallback: MpcBrowserErrorCallback?
) {
    let value = browser(browserPtr)
    let delegate = BrowserDelegateBox(
        context: context,
        foundCallback: foundCallback,
        lostCallback: lostCallback,
        errorCallback: errorCallback
    )
    value.delegate = delegate
    browserDelegatesLock.lock()
    browserDelegates[ObjectIdentifier(value)] = delegate
    browserDelegatesLock.unlock()
}

@_cdecl("mpc_browser_clear_delegate")
public func mpc_browser_clear_delegate(_ browserPtr: UnsafeMutableRawPointer) {
    let value = browser(browserPtr)
    value.delegate = nil
    browserDelegatesLock.lock()
    browserDelegates.removeValue(forKey: ObjectIdentifier(value))
    browserDelegatesLock.unlock()
}
