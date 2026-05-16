// swiftlint:disable identifier_name
import Foundation
import MultipeerConnectivity

let MPC_ERROR_KIND_INVALID_ARGUMENT: Int32 = 1
let MPC_ERROR_KIND_OPERATION_FAILED: Int32 = 2
let MPC_ERROR_KIND_FRAMEWORK: Int32 = 3

final class BridgeErrorBox: NSObject {
    let kind: Int32
    let domain: String
    let code: Int32
    let message: String

    init(kind: Int32, domain: String = "", code: Int32 = 0, message: String) {
        self.kind = kind
        self.domain = domain
        self.code = code
        self.message = message
    }
}

func writeErrorOut(
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?,
    _ error: BridgeErrorBox
) {
    errorOut?.pointee = Unmanaged.passRetained(error).toOpaque()
}

func writeInvalidArgument(
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?,
    _ message: String
) {
    writeErrorOut(errorOut, BridgeErrorBox(kind: MPC_ERROR_KIND_INVALID_ARGUMENT, message: message))
}

func writeNSError(
    _ errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?,
    _ error: Error
) {
    let nsError = error as NSError
    let kind = nsError.domain == MCErrorDomain ? MPC_ERROR_KIND_FRAMEWORK : MPC_ERROR_KIND_OPERATION_FAILED
    writeErrorOut(
        errorOut,
        BridgeErrorBox(
            kind: kind,
            domain: nsError.domain,
            code: Int32(nsError.code),
            message: nsError.localizedDescription
        )
    )
}

func bridgeError(_ ptr: UnsafeMutableRawPointer) -> BridgeErrorBox {
    Unmanaged<BridgeErrorBox>.fromOpaque(ptr).takeUnretainedValue()
}

func retainedNSError(_ error: Error?) -> UnsafeMutableRawPointer? {
    guard let error else { return nil }
    let nsError = error as NSError
    return Unmanaged.passRetained(
        BridgeErrorBox(
            kind: nsError.domain == MCErrorDomain ? MPC_ERROR_KIND_FRAMEWORK : MPC_ERROR_KIND_OPERATION_FAILED,
            domain: nsError.domain,
            code: Int32(nsError.code),
            message: nsError.localizedDescription
        )
    ).toOpaque()
}

@_cdecl("mpc_error_kind")
public func mpc_error_kind(_ error: UnsafeMutableRawPointer?) -> Int32 {
    guard let error else { return MPC_ERROR_KIND_OPERATION_FAILED }
    return bridgeError(error).kind
}

@_cdecl("mpc_error_code")
public func mpc_error_code(_ error: UnsafeMutableRawPointer?) -> Int32 {
    guard let error else { return 0 }
    return bridgeError(error).code
}

@_cdecl("mpc_error_domain")
public func mpc_error_domain(_ error: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let error else { return nil }
    return ffiString(bridgeError(error).domain)
}

@_cdecl("mpc_error_description")
public func mpc_error_description(_ error: UnsafeMutableRawPointer?) -> UnsafeMutablePointer<CChar>? {
    guard let error else { return nil }
    return ffiString(bridgeError(error).message)
}

@_cdecl("mpc_mc_error_domain")
public func mpc_mc_error_domain() -> UnsafeMutablePointer<CChar>? {
    ffiString(MCErrorDomain)
}
