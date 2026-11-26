// couch and the rest of the Mac: Now Playing (the menu bar / Control Centre
// widget, the media keys, AirPods), and staying awake while a film plays.
#import <Cocoa/Cocoa.h>
#import <MediaPlayer/MediaPlayer.h>
#import <IOKit/pwr_mgt/IOPMLib.h>

typedef void (*couch_press_fn)(int);
static couch_press_fn g_press;
static IOPMAssertionID g_awake = 0;

// presses: 0 toggle, 1 play, 2 pause, 3 next, 4 back 10 s, 5 stop, 6 forward 10 s
void couch_mac_np_commands(couch_press_fn f) {
    g_press = f;
    dispatch_async(dispatch_get_main_queue(), ^{
        MPRemoteCommandCenter *c = [MPRemoteCommandCenter sharedCommandCenter];
        [c.togglePlayPauseCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(0); return MPRemoteCommandHandlerStatusSuccess; }];
        [c.playCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(1); return MPRemoteCommandHandlerStatusSuccess; }];
        [c.pauseCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(2); return MPRemoteCommandHandlerStatusSuccess; }];
        [c.nextTrackCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(3); return MPRemoteCommandHandlerStatusSuccess; }];
        [c.stopCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(5); return MPRemoteCommandHandlerStatusSuccess; }];
        c.skipBackwardCommand.preferredIntervals = @[ @10 ];
        [c.skipBackwardCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(4); return MPRemoteCommandHandlerStatusSuccess; }];
        c.skipForwardCommand.preferredIntervals = @[ @10 ];
        [c.skipForwardCommand addTargetWithHandler:^MPRemoteCommandHandlerStatus(MPRemoteCommandEvent *e) { if (g_press) g_press(6); return MPRemoteCommandHandlerStatusSuccess; }];
    });
}

// what's playing (open = 0: nothing, the widget goes away)
void couch_mac_np_set(int open, int paused, const char *title, const char *sub, double pos, double duration, int has_next) {
    NSString *t = title ? [NSString stringWithUTF8String:title] : @"";
    NSString *s = sub ? [NSString stringWithUTF8String:sub] : @"";
    dispatch_async(dispatch_get_main_queue(), ^{
        MPNowPlayingInfoCenter *np = [MPNowPlayingInfoCenter defaultCenter];
        [MPRemoteCommandCenter sharedCommandCenter].nextTrackCommand.enabled = has_next != 0;
        if (!open) {
            np.nowPlayingInfo = nil;
            np.playbackState = MPNowPlayingPlaybackStateStopped;
            return;
        }
        NSMutableDictionary *info = [np.nowPlayingInfo mutableCopy] ?: [NSMutableDictionary dictionary];
        info[MPMediaItemPropertyTitle] = t;
        info[MPMediaItemPropertyArtist] = s;
        info[MPMediaItemPropertyPlaybackDuration] = @(duration);
        info[MPNowPlayingInfoPropertyElapsedPlaybackTime] = @(pos);
        info[MPNowPlayingInfoPropertyPlaybackRate] = @(paused ? 0.0 : 1.0);
        info[MPNowPlayingInfoPropertyMediaType] = @(MPNowPlayingInfoMediaTypeVideo);
        np.nowPlayingInfo = info;
        np.playbackState = paused ? MPNowPlayingPlaybackStatePaused : MPNowPlayingPlaybackStatePlaying;
    });
}

// the title's picture (JPEG / PNG bytes)
void couch_mac_np_art(const unsigned char *bytes, size_t len) {
    NSData *d = [NSData dataWithBytes:bytes length:len];
    dispatch_async(dispatch_get_main_queue(), ^{
        NSImage *img = [[NSImage alloc] initWithData:d];
        if (!img)
            return;
        MPMediaItemArtwork *art = [[MPMediaItemArtwork alloc] initWithBoundsSize:img.size requestHandler:^NSImage *(CGSize size) { return img; }];
        MPNowPlayingInfoCenter *np = [MPNowPlayingInfoCenter defaultCenter];
        NSMutableDictionary *info = [np.nowPlayingInfo mutableCopy] ?: [NSMutableDictionary dictionary];
        info[MPMediaItemPropertyArtwork] = art;
        np.nowPlayingInfo = info;
    });
}

// playing: the Mac doesn't sleep or dim the screen (paused or closed: it may)
void couch_mac_stay_awake(int on) {
    if (on && !g_awake) {
        IOPMAssertionCreateWithName(kIOPMAssertionTypeNoDisplaySleep, kIOPMAssertionLevelOn, CFSTR("couch is playing a film"), &g_awake);
    } else if (!on && g_awake) {
        IOPMAssertionRelease(g_awake);
        g_awake = 0;
    }
}
