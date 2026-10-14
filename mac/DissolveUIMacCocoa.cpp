//-------------------------------------------------------------------------------
//	Includes
//-------------------------------------------------------------------------------

#include "PIDefines.h"
#include "Dissolve.h"
#include "DissolveUI.h"
#include "DialogUtilities.h"
#include "FilterBigDocument.h"

#include <Cocoa/Cocoa.h>
#include "DissolveController.h"


//-------------------------------------------------------------------------------
//
// DoUI
//
// Pop our UI, the UI is using the globals and the person calling this is responsible
// to reset the globals to last used accordingly
//
// Return: 1 for ok (true) and 0 for cancel
//-------------------------------------------------------------------------------
extern DissolveController *gDissolveController;

Boolean DoUI (void) 
{
	// call function to initialize Cocoa
    OSStatus err = initializeCocoaDissolve();

	// call function to show window
	// it's not an error 1 = ok, 0 = cancel
    if (!err)
		err = orderWindowFrontDissolve();
	else
		err = 0; // simulate cancel

	sPSUIHooks.Unload();
	
	return err;
}

//-------------------------------------------------------------------------------
//
// DoAbout
//
// Pop a simple about box for this plug in.
//
// NOTE:	The global gFilterRecord is NOT a FilterRecord*. You must cast it to
//			an AboutRecord*. See PIAbout.h
//
//-------------------------------------------------------------------------------
void DoAbout(void)
{
	NSLog(@"DoAbout is: %d\n",AboutID);

	NSAlert *alert = [[NSAlert alloc] init];
	
	[alert addButtonWithTitle:@"OK"];
	
	[alert addButtonWithTitle:@"http://www.getartcraft.com"];
	
	[alert addButtonWithTitle:@"http://partners.getartcraft.com"];

	[alert setMessageText:@"About Dissolve"];
	
	[alert setInformativeText:@"An example plug-in filter module for PhotoCraft."];
	
	[alert setAlertStyle:NSWarningAlertStyle];
	
	int buttonPressed = [alert runModal];
	
	if ( buttonPressed == NSAlertFirstButtonReturn) {
		NSLog(@"DoAbout ok clicked.\n");
	} else if (buttonPressed == NSAlertSecondButtonReturn) {
		sPSFileList->BrowseUrl("http://www.getartcraft.com");
	} else if (buttonPressed == NSAlertThirdButtonReturn) {
		sPSFileList->BrowseUrl("http://partners.getartcraft.com");
	}

	[alert release];
}

// end DissolveUIMacCocoa.cpp 