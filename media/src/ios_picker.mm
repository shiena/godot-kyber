// SPDX-FileCopyrightText: 2026 KOGA Mitsuhiro
// SPDX-License-Identifier: MIT

// UIKit owns presentation; Godot polls the result without receiving UI callbacks.
#import <UIKit/UIKit.h>
#import <UniformTypeIdentifiers/UniformTypeIdentifiers.h>
#include <mutex>
#include <string>
static std::mutex picker_mutex;
static bool picker_active = false, picker_ready = false;
static std::string picker_result;
static void finish(NSString* result) {
    std::lock_guard<std::mutex> lock(picker_mutex);
    picker_result = result.UTF8String ?: "error:Invalid picker result";
    picker_active = false;
    picker_ready = true;
}
@interface KyberVideoPickerDelegate : NSObject <UIDocumentPickerDelegate>
@end
@implementation KyberVideoPickerDelegate
- (void)documentPickerWasCancelled:(UIDocumentPickerViewController*)controller {
    finish(@"cancel:");
}
- (void)documentPicker:(UIDocumentPickerViewController*)controller didPickDocumentsAtURLs:(NSArray<NSURL*>*)urls {
    NSURL* source = urls.firstObject;
    if (!source) { finish(@"cancel:"); return; }
    BOOL scoped = [source startAccessingSecurityScopedResource];
    NSURL* directory = [[[NSFileManager defaultManager] URLsForDirectory:NSCachesDirectory inDomains:NSUserDomainMask] firstObject];
    NSURL* destination = [directory URLByAppendingPathComponent:[NSString stringWithFormat:@"kyber-%@.%@", NSUUID.UUID.UUIDString, source.pathExtension]];
    NSError* error = nil;
    BOOL copied = [[NSFileManager defaultManager] copyItemAtURL:source toURL:destination error:&error];
    if (scoped) [source stopAccessingSecurityScopedResource];
    if (copied) finish([@"ok:" stringByAppendingString:destination.path]);
    else finish([@"error:" stringByAppendingString:error.localizedDescription ?: @"Cannot copy selected video"]);
}
@end
extern "C" bool kg_ios_picker_open() {
    {
        std::lock_guard<std::mutex> lock(picker_mutex);
        if (picker_active) return false;
        picker_active = true; picker_ready = false;
    }
    dispatch_async(dispatch_get_main_queue(), ^{
        static KyberVideoPickerDelegate* delegate = [KyberVideoPickerDelegate new];
        UIViewController* presenter = nil;
        for (UIScene* scene in UIApplication.sharedApplication.connectedScenes) {
            if (scene.activationState != UISceneActivationStateForegroundActive || ![scene isKindOfClass:UIWindowScene.class]) continue;
            for (UIWindow* window in ((UIWindowScene*)scene).windows) if (window.isKeyWindow) presenter = window.rootViewController;
        }
        if (!presenter) presenter = UIApplication.sharedApplication.keyWindow.rootViewController;
        while (presenter.presentedViewController) presenter = presenter.presentedViewController;
        if (!presenter) { finish(@"error:No active iOS window"); return; }
        UIDocumentPickerViewController* picker = [[UIDocumentPickerViewController alloc] initForOpeningContentTypes:@[UTTypeMovie] asCopy:YES];
        picker.delegate = delegate;
        picker.allowsMultipleSelection = NO;
        [presenter presentViewController:picker animated:YES completion:nil];
    });
    return true;
}
extern "C" const char* kg_ios_picker_result() {
    static thread_local std::string result;
    std::lock_guard<std::mutex> lock(picker_mutex);
    if (!picker_ready) return nullptr;
    result = picker_result;
    picker_ready = false;
    return result.c_str();
}
