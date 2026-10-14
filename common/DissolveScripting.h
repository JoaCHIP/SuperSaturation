#ifndef _DISSOLVESCRIPTING_H
#define _DISSOLVESCRIPTING_H

#include "PIDefines.h"
#include "PIActions.h"
#include "PITerminology.h"

#ifndef Rez
#include "Dissolve.h"
#endif

#define vendorName			"JoaCHIP"
#define plugInName			"Super Saturation"
#define plugInAETEComment	"Refined Saturation Control"
#define plugInSuiteID		'sdk1'
#define plugInClassID		plugInSuiteID
#define plugInEventID		plugInClassID
#define plugInUniqueID		"F9A5705A-238A-4EF0-955F-C05DE4A64706"

#define plugInCopyrightYear	"2022"
#define plugInDescription	"Smoother looking saturation and gamma curves."

// UI parameters
#define keyLiveliness		'livE'
#define keyWarmth			'warM'
#define keyDisposition 		'disP'
#define keyIgnoreSelection	'ignS'
#define typeMood			'mooD'
#define dispositionClear	'moD0'
#define dispositionCool		'moD1'
#define dispositionHot		'moD2'
#define dispositionSick		'moD3'

#ifndef Rez
OSErr ReadScriptParameters(Boolean* displayDialog);
OSErr WriteScriptParameters(void);
int16 ScriptToDialog(int32 script);
int32 DialogToScript(int16 dialog);
#endif
#endif
