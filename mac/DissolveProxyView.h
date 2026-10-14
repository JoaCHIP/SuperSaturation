#import <Cocoa/Cocoa.h>

#include "PSIntTypes.h"


typedef enum

	{
	ZoomCommandUninited,
	ZoomCommandHigh,
	ZoomCommandLow
	
	} ZoomCommandState;

/* Make sure this is unique to you and everyone you might encounter, search for
"Preventing Name Conflicts" or use this link
http://developer.apple.com/mac/library/documentation/UserExperience/Conceptual/PreferencePanes/Tasks/Conflicts.html
*/
@interface DissolveProxyView : NSView {
	NSColor * currentColor;
	ZoomCommandState currentZoomState;
}

- (CGFloat)getCurrentScaleFactor;
- (void)setDispositionColor:(int16)newColor;
- (BOOL)isFlipped;


@end

// end DissolveProxyView.h
