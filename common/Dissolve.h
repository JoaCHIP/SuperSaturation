#ifndef _DISSOLVE_H
#define _DISSOLVE_H

#include "PIDefines.h"
#include "PITypes.h"
#include "PIAbout.h"
#include "PIFilter.h"
#include "PIUtilities.h"

typedef struct Parameters
{
	int16 saturation;
	int16 liveliness;
	int16 gamma;
	int16 contrast;
	int16 warmth;
	int16 hueshift;
	int16 percent;
	int16 disposition;
	Boolean ignoreSelection;
} Parameters, * ParametersPtr;

typedef struct TempParameters
{
	float Saturation = 0;
	float Liveliness = 0;
	float Gamma1 = 0;
	float Gamma2 = 0;
	float Contrast = 0;
	float Warmth = 0;
	float HueShift = 0;
};

typedef struct Data
{
	FilterColor color;
	FilterColor colorArray[4];
	Boolean queryForParameters;
	VRect proxyRect;
	float scaleFactor;
	BufferID proxyBufferID;
	Ptr proxyBuffer;
	int32 proxyWidth;
	int32 proxyHeight;
	int32 proxyPlaneSize;
} Data;

typedef struct ColorRGBA
{
	float r;
	float g;
	float b;
	float a;
} ColorRGBA;

typedef struct ColorHSV
{
	float h;
	float s;
	float v;
} ColorHSV;

extern FilterRecord* gFilterRecord;
extern Data* gData;
extern int16* gResult;
extern Parameters* gParams;
extern SPBasicSuite* sSPBasic;

void SetupFilterRecordForProxy(void);
void CopyColor(FilterColor& destination, const FilterColor& source);
void SetColor(FilterColor& destination, const uint8 a, const uint8 b, const uint8 c, const uint8 d);
void CreateProxyBuffer(void);
extern "C" void ResetProxyBuffer(void);
extern "C" void UpdateProxyBuffer(void);
void DeleteProxyBuffer(void);
int32 DisplayPixelsMode(int16 mode);
TempParameters PrecalcParameters() noexcept;
ColorRGBA ProcessPixel(ColorRGBA inputColor, TempParameters temp) noexcept;
ColorHSV RGBtoHSV(ColorRGBA in) noexcept;
ColorRGBA HSVtoRGB(ColorHSV in) noexcept;

#endif
