#import <AppKit/AppKit.h>

NSString *get_default_app_bundle_for_file(NSString *file_path) {
    NSURL *fileUrl = [NSURL fileURLWithPath:file_path];
    NSURL *appUrl = [[NSWorkspace sharedWorkspace] URLForApplicationToOpenURL:fileUrl];
    if (!appUrl) {
        return nil;
    }

    NSBundle *appBundle = [NSBundle bundleWithURL:appUrl];
    if (!appBundle) {
        return nil;
    }
    return [appBundle bundleIdentifier];
}
