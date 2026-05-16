// swiftlint:disable identifier_name
import Dispatch
import Foundation
import MultipeerConnectivity

let MPC_OK: Int32 = 0
let MPC_INVALID_ARGUMENT: Int32 = -1
let MPC_OPERATION_FAILED: Int32 = -2

final class ObjectBox: NSObject {
    let value: AnyObject

    init(_ value: AnyObject) {
        self.value = value
    }
}

func ffiString(_ string: String?) -> UnsafeMutablePointer<CChar>? {
    guard let string else { return nil }
    return string.withCString { strdup($0) }
}

func retainObject(_ object: AnyObject) -> UnsafeMutableRawPointer {
    Unmanaged.passRetained(ObjectBox(object)).toOpaque()
}

func unbox<T: AnyObject>(_ ptr: UnsafeMutableRawPointer, as type: T.Type = T.self) -> T {
    let box = Unmanaged<ObjectBox>.fromOpaque(ptr).takeUnretainedValue()
    guard let value = box.value as? T else {
        fatalError("Unexpected boxed object type")
    }
    return value
}

func copyRawData(_ bytes: UnsafeRawPointer?, _ count: Int) -> Data {
    guard let bytes, count > 0 else { return Data() }
    return Data(bytes: bytes, count: count)
}

func copyCString(_ string: UnsafePointer<CChar>) -> String {
    String(cString: string)
}

func dataBuffer(_ data: Data) -> UnsafeMutableRawPointer? {
    guard !data.isEmpty else { return nil }
    let buffer = UnsafeMutableRawPointer.allocate(byteCount: data.count, alignment: 1)
    data.copyBytes(to: buffer.assumingMemoryBound(to: UInt8.self), count: data.count)
    return buffer
}

func decodeDiscoveryInfo(
    _ discoveryInfoJson: UnsafePointer<CChar>?,
    errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> [String: String]? {
    guard let discoveryInfoJson else { return nil }
    let string = copyCString(discoveryInfoJson)
    guard !string.isEmpty else { return nil }
    guard let data = string.data(using: .utf8),
          let parsed = try? JSONSerialization.jsonObject(with: data, options: []),
          let dict = parsed as? [String: String]
    else {
        writeInvalidArgument(errorOut, "discoveryInfo must be a JSON object of string pairs")
        return nil
    }
    return dict
}

func jsonCString(for discoveryInfo: [String: String]?) -> UnsafeMutablePointer<CChar>? {
    guard let discoveryInfo else { return nil }
    guard JSONSerialization.isValidJSONObject(discoveryInfo) else { return nil }
    guard let data = try? JSONSerialization.data(withJSONObject: discoveryInfo, options: []),
          let string = String(data: data, encoding: .utf8)
    else {
        return nil
    }
    return ffiString(string)
}

func writeRetainedArray<T: AnyObject>(
    _ values: [T],
    outArray: UnsafeMutablePointer<UnsafeMutableRawPointer?>,
    outCount: UnsafeMutablePointer<Int>
) {
    outCount.pointee = values.count
    guard !values.isEmpty else {
        outArray.pointee = nil
        return
    }
    let buffer = UnsafeMutablePointer<UnsafeMutableRawPointer?>.allocate(capacity: values.count)
    for (index, value) in values.enumerated() {
        buffer[index] = retainObject(value)
    }
    outArray.pointee = UnsafeMutableRawPointer(buffer)
}

func rawObjectArray(_ items: UnsafePointer<UnsafeMutableRawPointer?>?, count: Int) -> [AnyObject]? {
    guard let items, count > 0 else { return nil }
    var array: [AnyObject] = []
    array.reserveCapacity(count)
    for index in 0 ..< count {
        guard let raw = items.advanced(by: index).pointee else { continue }
        let object = Unmanaged<AnyObject>.fromOpaque(raw).takeUnretainedValue()
        array.append(object)
    }
    return array.isEmpty ? nil : array
}

func boxedObjectArray(_ items: UnsafePointer<UnsafeMutableRawPointer?>?, count: Int) -> [AnyObject]? {
    guard let items, count > 0 else { return nil }
    var array: [AnyObject] = []
    array.reserveCapacity(count)
    for index in 0 ..< count {
        guard let raw = items.advanced(by: index).pointee else { continue }
        let object = Unmanaged<ObjectBox>.fromOpaque(raw).takeUnretainedValue().value
        array.append(object)
    }
    return array.isEmpty ? nil : array
}

func validateServiceType(
    _ serviceType: String,
    errorOut: UnsafeMutablePointer<UnsafeMutableRawPointer?>?
) -> Bool {
    guard !serviceType.isEmpty else {
        writeInvalidArgument(errorOut, "service type must not be empty")
        return false
    }
    guard serviceType.count <= 15 else {
        writeInvalidArgument(errorOut, "service type must be at most 15 ASCII characters")
        return false
    }
    let valid = serviceType.utf8.allSatisfy { byte in
        (byte >= 97 && byte <= 122) || (byte >= 48 && byte <= 57) || byte == 45
    }
    guard valid else {
        writeInvalidArgument(
            errorOut,
            "service type must contain only lowercase ASCII letters, digits, or hyphens"
        )
        return false
    }
    return true
}

func onMain<T>(_ work: @escaping () -> T) -> T {
    if Thread.isMainThread {
        return work()
    }
    return DispatchQueue.main.sync(execute: work)
}

@_cdecl("mpc_string_free")
public func mpc_string_free(_ string: UnsafeMutablePointer<CChar>?) {
    guard let string else { return }
    free(string)
}

@_cdecl("mpc_bytes_free")
public func mpc_bytes_free(_ ptr: UnsafeMutableRawPointer?) {
    ptr?.deallocate()
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
    return Unmanaged.passRetained(object).toOpaque()
}

@_cdecl("mpc_ptr_array_free")
public func mpc_ptr_array_free(_ ptr: UnsafeMutableRawPointer?) {
    guard let ptr else { return }
    ptr.assumingMemoryBound(to: UnsafeMutableRawPointer?.self).deallocate()
}
