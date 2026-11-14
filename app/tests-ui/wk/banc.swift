// app/tests-ui/wk/banc.swift — Banc de rendu dans le vrai WKWebView de macOS (section 11.1).
//
// Ouvre une fenêtre de 1280 × 800 points avec une WKWebView, comme l'app ; injecte
// avant tout script le fichier donné (imitation de l'IPC et flux du cœur, produit
// par perf.spec.ts), charge l'interface (vite preview), attend 3 s, puis appelle
// __pbReplay() et écrit « DEBUT » puis « FIN <secondes> » sur la sortie standard.
// Piloté par banc.sh, qui mesure le CPU des processus WebKit de ce programme.
//   swiftc -O banc.swift -o banc && ./banc <url> <script.js>
import AppKit
import WebKit

let args = CommandLine.arguments
guard args.count >= 3, let url = URL(string: args[1]),
      let script = try? String(contentsOfFile: args[2], encoding: .utf8) else {
    FileHandle.standardError.write("usage : banc <url> <script.js>\n".data(using: .utf8)!)
    exit(2)
}

final class Delegate: NSObject, NSApplicationDelegate, WKNavigationDelegate {
    let url: URL
    let script: String
    var window: NSWindow!
    var web: WKWebView!

    init(url: URL, script: String) {
        self.url = url
        self.script = script
    }

    func applicationDidFinishLaunching(_ note: Notification) {
        let config = WKWebViewConfiguration()
        config.userContentController.addUserScript(
            WKUserScript(source: script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        web = WKWebView(frame: NSRect(x: 0, y: 0, width: 1280, height: 800), configuration: config)
        web.navigationDelegate = self
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: 1280, height: 800),
                          styleMask: [.titled, .closable, .miniaturizable], backing: .buffered, defer: false)
        window.title = "Banc de rendu Prebench"
        window.contentView = web
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        web.load(URLRequest(url: url))
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
            print("DEBUT"); fflush(stdout)
            // BANC_REDUIRE=1 : fenêtre réduite dans le Dock pendant le rejeu.
            if ProcessInfo.processInfo.environment["BANC_REDUIRE"] == "1" {
                self.window.miniaturize(nil)
                DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                    webView.evaluateJavaScript("document.visibilityState") { v, _ in
                        print("VISIBILITE page \(v ?? "?")"); fflush(stdout)
                    }
                }
            }
            webView.callAsyncJavaScript("return await window.__pbReplay();", arguments: [:],
                                        in: nil, in: .page) { result in
                switch result {
                case .success(let v): print("FIN \(v ?? "?")")
                    // Capture de contrôle : la paillasse est-elle bien affichée ?
                    webView.takeSnapshot(with: nil) { image, _ in
                        if let tiff = image?.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
                           let png = rep.representation(using: .png, properties: [:]) {
                            try? png.write(to: URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("prebench-banc.png"))
                        }
                    }
                case .failure(let e): print("ERREUR \(e)")
                }
                fflush(stdout)
                // Laisse à banc.sh le temps de relever le CPU des processus WebKit.
                DispatchQueue.main.asyncAfter(deadline: .now() + 5) { exit(0) }
            }
        }
    }
}

let app = NSApplication.shared
let delegate = Delegate(url: url, script: script)
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
