#import <Cocoa/Cocoa.h>
#import <Foundation/Foundation.h>

@interface WarpDockTilePlugIn : NSObject <NSDockTilePlugIn>
{
    id iconChangedObserver;
    id defaultsObserver;
}

@property(strong) id iconChangedObserver;
@property(strong) id defaultsObserver;
@property(weak) NSDockTile *observedDockTile;
@end
