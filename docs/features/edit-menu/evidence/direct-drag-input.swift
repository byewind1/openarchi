import AppKit
import CoreGraphics
let args = CommandLine.arguments
let pid = pid_t(args[1])!
guard let app = NSRunningApplication(processIdentifier: pid), ["com.graphisoft.archicad29", "com.openarchi.desktop"].contains(app.bundleIdentifier ?? "") else { fatalError("unexpected target") }
app.activate(options: [.activateIgnoringOtherApps])
Thread.sleep(forTimeInterval: 0.15)
guard NSWorkspace.shared.frontmostApplication?.processIdentifier == pid else { fatalError("target did not become foreground") }
let source = CGEventSource(stateID: .hidSystemState)
let flags = CGEventFlags(rawValue: UInt64(args.last!)!)
if ["move", "click", "relmove", "relclick", "reldown", "reldrag", "relup", "relstartdrag"].contains(args[2]) {
 let windows = CGWindowListCopyWindowInfo(.optionOnScreenOnly, kCGNullWindowID) as? [[String:Any]] ?? []
 let bounds = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int) == Int(pid) }.compactMap { $0[kCGWindowBounds as String] as? [String:Double] }.filter { ($0["Width"] ?? 0) > 500 }.first!
 let point = CGPoint(x: Double(args[3])! + (args[2].hasPrefix("rel") ? bounds["X"]! : 0), y: Double(args[4])! + (args[2].hasPrefix("rel") ? bounds["Y"]! : 0))
 guard CGRect(x: bounds["X"]!, y: bounds["Y"]!, width: bounds["Width"]!, height: bounds["Height"]!).contains(point) else { fatalError("point outside target window") }
 if args[2] == "relstartdrag" {
  let down = CGEvent(mouseEventSource: source, mouseType: .leftMouseDown, mouseCursorPosition: point, mouseButton: .left)!; down.flags = flags; down.post(tap: .cghidEventTap)
  Thread.sleep(forTimeInterval:0.01)
  for step in 1...10 {
   let next = CGPoint(x:point.x + Double(step)*3, y:point.y + Double(step)*4)
   let event = CGEvent(mouseEventSource:source,mouseType:.leftMouseDragged,mouseCursorPosition:next,mouseButton:.left)!; event.flags=flags;event.post(tap:.cghidEventTap);Thread.sleep(forTimeInterval:0.01)
  }
  exit(0)
 }
 let move = CGEvent(mouseEventSource: source, mouseType: args[2] == "reldrag" ? .leftMouseDragged : args[2] == "reldown" ? .leftMouseDown : args[2] == "relup" ? .leftMouseUp : .mouseMoved, mouseCursorPosition: point, mouseButton: .left)!
 move.flags = flags; move.post(tap: .cghidEventTap)
 if args[2].hasSuffix("click") {
  Thread.sleep(forTimeInterval: 0.08)
  for type in [CGEventType.leftMouseDown, .leftMouseUp] {
   let e = CGEvent(mouseEventSource: source, mouseType: type, mouseCursorPosition: point, mouseButton: .left)!; e.flags=flags; e.post(tap: .cghidEventTap); Thread.sleep(forTimeInterval:0.06)
  }
 }
} else {
 let code = CGKeyCode(args[3])!
 for down in (args[2] == "moddown" ? [true] : args[2] == "modup" ? [false] : [true, false]) {
  let e = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down)!; e.flags = args[2] == "modifier" && !down ? [] : flags; if ["modifier", "moddown", "modup"].contains(args[2]) { e.type = .flagsChanged }; e.post(tap: .cghidEventTap); Thread.sleep(forTimeInterval:0.06)
 }
}
Thread.sleep(forTimeInterval:0.15)
