#ifndef _DISSOLVEUI_H
#define _DISSOLVEUI_H

#include "PITypes.h"
#include "DialogUtilities.h"

const int16 kDOK = 1;
const int16 kDCancel = 2;
const int16 kDEditText = 3;
const int16 kDProxyItem = 5;
const int16 kDFirstRadio = 7;
const int16 kDLastRadio = 10;
const int16 kDEntireImage = 11;
const int16 kDSaturation = 100;
const int16 kDLiveliness = 101;
const int16 kDGamma = 102;
const int16 kDContrast = 103;
const int16 kDWarmth = 104;
const int16 kDHueShift = 105;

Boolean DoUI(void);
void DoAbout(void);
#endif
// end DissolveUI.h
