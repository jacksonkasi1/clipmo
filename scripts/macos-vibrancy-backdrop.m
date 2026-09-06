// A deterministic desktop fixture for hosted-runner screenshots only.
#import <AppKit/AppKit.h>

@interface ColorBackdrop : NSView
@end
@implementation ColorBackdrop
- (void)drawRect:(NSRect)dirtyRect {
    CGFloat width = self.bounds.size.width / 12.0;
    for (NSInteger stripe = 0; stripe < 12; stripe++) {
        [(stripe % 2 == 0 ? [NSColor colorWithSRGBRed:0.05 green:0.35 blue:0.95 alpha:1]
                          : [NSColor colorWithSRGBRed:0.95 green:0.25 blue:0.12 alpha:1]) setFill];
        NSRectFill(NSMakeRect(stripe * width, 0, width + 1, self.bounds.size.height));
    }
}
@end

int main(void) {
    @autoreleasepool {
        NSApplication *app = NSApplication.sharedApplication;
        if (NSWorkspace.sharedWorkspace.accessibilityDisplayShouldReduceTransparency) {
            fprintf(stderr, "Reduce Transparency is still enabled on the CI runner\n");
            return 1;
        }
        [app setActivationPolicy:NSApplicationActivationPolicyAccessory];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSScreen.mainScreen.frame
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        window.contentView = [[ColorBackdrop alloc] initWithFrame:window.contentView.bounds];
        window.level = NSNormalWindowLevel - 1;
        window.ignoresMouseEvents = YES;
        [window orderFrontRegardless];
        [app run];
    }
}
