// app/tests-ui/wk/banc.swift — Banc de rendu dans le vrai WKWebView de macOS (section 11.1).
//
// Ouvre une fenêtre de 1200 × 800 points (taille de l'app ; BANC_LARGEUR et
// BANC_HAUTEUR pour une autre) avec une WKWebView, comme l'app ; injecte
// avant tout script le fichier donné (imitation de l'IPC et flux du cœur, produit
// par perf.spec.ts), charge l'interface (vite preview), attend 3 s, puis appelle
// __pbReplay() et écrit « DEBUT » puis « FIN <secondes> » sur la sortie standard.
// Piloté par banc.sh, qui mesure le CPU des processus WebKit de ce programme.
// Avec un troisième argument (wk-events.json), le flux est rejoué par ce
// programme, comme le fait Tauri : un script évalué par le processus natif
// pour chaque événement (evaluateJavaScript), rien quand la fenêtre est réduite.
//   swiftc -O banc.swift -o banc && ./banc <url> <script.js> [wk-events.json]
import AppKit
import WebKit

let args = CommandLine.arguments
let env = ProcessInfo.processInfo.environment
let width = Double(env["BANC_LARGEUR"] ?? "") ?? 1200
let height = Double(env["BANC_HAUTEUR"] ?? "") ?? 800
let eventsPath = args.count >= 4 ? args[3] : nil
let checkErrors = env["BANC_ERREURS"] == "1"
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
    var reported = false
    var activity: NSObjectProtocol?
    var timer: DispatchSourceTimer?

    init(url: URL, script: String) {
        self.url = url
        self.script = script
    }

    func applicationDidFinishLaunching(_ note: Notification) {
        let config = WKWebViewConfiguration()
        config.userContentController.addUserScript(
            WKUserScript(source: script, injectionTime: .atDocumentStart, forMainFrameOnly: true))
        web = WKWebView(frame: NSRect(x: 0, y: 0, width: width, height: height), configuration: config)
        web.navigationDelegate = self
        window = NSWindow(contentRect: NSRect(x: 80, y: 80, width: width, height: height),
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
            if let path = eventsPath {
                self.replayNative(path)
                return
            }
            webView.callAsyncJavaScript("return await window.__pbReplay();", arguments: [:],
                                        in: nil, in: .page) { result in
                switch result {
                case .success(let v):
                    webView.evaluateJavaScript("JSON.stringify(window.__pbSent)") { e, _ in
                        print("ENVOIS \(e ?? "?")"); fflush(stdout)
                    }
                    print("FIN \(v)")
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

extension Delegate {
    /// Rejeu par le processus natif, comme Tauri (evaluateJavaScript par événement).
    func replayNative(_ path: String) {
        guard let data = FileManager.default.contents(atPath: path),
              let root = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let scripts = root["scripts"] as? [[String: Any]] else {
            print("ERREUR flux illisible : \(path)"); fflush(stdout); exit(1)
        }
        let duration = (root["duration"] as? Double) ?? 20000
        // Minuteries à l'heure, comme le cœur de l'app qui émet depuis ses
        // propres fils : sans cela, App Nap regroupe les envois du banc.
        self.activity = ProcessInfo.processInfo.beginActivity(
            options: [.userInitiated, .latencyCritical], reason: "rejeu du banc")
        let start = DispatchTime.now()
        web.evaluateJavaScript("window.__pbEmit('sim:status', { state: 'running', message: 'En cours' })")
        // Une seule minuterie, reprogrammée pour chaque événement, tolérance de
        // 1 ms : des asyncAfter programmés d'avance reçoivent de GCD une tolérance
        // proportionnelle à leur délai et arrivent par paquets (mesuré : 13
        // images toutes les 0,5 à 0,9 s au bout de 10 s, soit 3 à 5 images
        // affichées par seconde au lieu de 15).
        let events = scripts.compactMap { e -> (Double, String)? in
            guard let t = e["t"] as? Double, let s = e["s"] as? String else { return nil }
            return (t, s)
        }
        var next = 0
        let timer = DispatchSource.makeTimerSource(queue: .main)
        self.timer = timer
        timer.setEventHandler {
            let now = Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1e6
            while next < events.count, events[next].0 <= now {
                let s = events[next].1
                next += 1
                if self.window.isMiniaturized { continue }
                // Sans gestionnaire de fin, comme Tauri (wry : eval d'un emit) ;
                // BANC_ERREURS=1 : gestionnaire qui signale la première erreur JS.
                if checkErrors {
                    self.web.evaluateJavaScript(s) { _, error in
                        if let error, !self.reported {
                            self.reported = true
                            print("ERREUR JS \(error)"); fflush(stdout)
                        }
                    }
                } else {
                    self.web.evaluateJavaScript(s, completionHandler: nil)
                }
            }
            if next < events.count {
                timer.schedule(deadline: start + .microseconds(Int(events[next].0 * 1000)), leeway: .milliseconds(1))
            } else {
                timer.cancel()
            }
        }
        timer.schedule(deadline: start, leeway: .milliseconds(1))
        timer.resume()
        DispatchQueue.main.asyncAfter(deadline: start + .milliseconds(Int(duration))) {
            let elapsed = Double(DispatchTime.now().uptimeNanoseconds - start.uptimeNanoseconds) / 1e9
            self.web.evaluateJavaScript("JSON.stringify(window.__pbSent)") { v, _ in
                print("ENVOIS \(v ?? "?")"); fflush(stdout)
            }
            print("FIN \(elapsed)"); fflush(stdout)
            self.web.takeSnapshot(with: nil) { image, _ in
                if let tiff = image?.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
                   let png = rep.representation(using: .png, properties: [:]) {
                    try? png.write(to: URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("prebench-banc.png"))
                }
            }
            DispatchQueue.main.asyncAfter(deadline: .now() + 5) { exit(0) }
        }
    }
}

let app = NSApplication.shared
let delegate = Delegate(url: url, script: script)
app.delegate = delegate
app.setActivationPolicy(.regular)
app.run()
