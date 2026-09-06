// Exercise the production PDF preview bundle using macOS's actual WKWebView.
#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>

@interface PDFTest : NSObject <WKURLSchemeHandler, WKScriptMessageHandler>
@property NSString *root;
@property WKWebView *webView;
@property NSWindow *window;
@end
@implementation PDFTest
- (void)webView:(WKWebView *)webView startURLSchemeTask:(id<WKURLSchemeTask>)task {
    NSString *path = [self.root stringByAppendingPathComponent:task.request.URL.path];
    NSData *data = [NSData dataWithContentsOfFile:path];
    if (!data) {
        [task didFailWithError:[NSError errorWithDomain:@"PDFTest" code:404 userInfo:@{NSLocalizedDescriptionKey:path}]];
        return;
    }
    NSDictionary *types = @{@"html":@"text/html", @"js":@"text/javascript", @"mjs":@"text/javascript", @"css":@"text/css", @"wasm":@"application/wasm"};
    NSURLResponse *response = [[NSURLResponse alloc] initWithURL:task.request.URL MIMEType:types[path.pathExtension] ?: @"application/octet-stream" expectedContentLength:data.length textEncodingName:nil];
    [task didReceiveResponse:response];
    [task didReceiveData:data];
    [task didFinish];
}
- (void)webView:(WKWebView *)webView stopURLSchemeTask:(id<WKURLSchemeTask>)task {}
- (void)userContentController:(WKUserContentController *)controller didReceiveScriptMessage:(WKScriptMessage *)message {
    NSDictionary *result = message.body;
    puts([[result description] UTF8String]);
    fflush(stdout);
    if (result[@"pass"]) exit([result[@"pass"] boolValue] ? 0 : 1);
}
@end
int main(int argc, const char *argv[]) {
    @autoreleasepool {
        if (argc != 2) return 2;
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
        PDFTest *test = [PDFTest new];
        test.root = [NSString stringWithUTF8String:argv[1]];
        WKWebViewConfiguration *configuration = [WKWebViewConfiguration new];
        [configuration setURLSchemeHandler:test forURLScheme:@"tauri"];
        [configuration.userContentController addScriptMessageHandler:test name:@"result"];
        NSString *diagnostics = @"window.addEventListener('error', e => window.webkit.messageHandlers.result.postMessage({pass:false,error:e.message})); window.addEventListener('unhandledrejection', e => window.webkit.messageHandlers.result.postMessage({pass:false,error:String(e.reason),stack:e.reason?.stack})); for (const level of ['warn','error']) { const original = console[level]; console[level] = (...args) => { original.apply(console,args); window.webkit.messageHandlers.result.postMessage({log:args.map(String).join(' ')}); }; }";
        [configuration.userContentController addUserScript:[[WKUserScript alloc] initWithSource:diagnostics injectionTime:WKUserScriptInjectionTimeAtDocumentStart forMainFrameOnly:YES]];
        test.webView = [[WKWebView alloc] initWithFrame:NSMakeRect(0, 0, 800, 1000) configuration:configuration];
        test.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 400, 500) styleMask:NSWindowStyleMaskTitled backing:NSBackingStoreBuffered defer:NO];
        test.window.title = @"Clipmo PDF compatibility test";
        test.window.contentView = test.webView;
        [test.window orderFront:nil];
        [test.webView loadRequest:[NSURLRequest requestWithURL:[NSURL URLWithString:@"tauri://localhost/pdf-webkit.html"]]];
        [NSTimer scheduledTimerWithTimeInterval:60 repeats:NO block:^(NSTimer *timer) { fputs("FAIL: PDF WebKit test timed out\n", stderr); exit(1); }];
        [NSApp run];
    }
}
