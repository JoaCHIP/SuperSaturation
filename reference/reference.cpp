// Reference harness: the original Photoshop filter's colour transform, compiled natively, used to
// check the Rust port (tests/reference.rs) against it.
//
// PrecalcParameters() and ProcessPixel() below are copied verbatim from common/Dissolve.cpp in
// https://github.com/JoaCHIP/SuperSaturation (MIT License, Copyright (c) 2026 JoaCHIP). Only the
// Photoshop SDK types they use are stubbed.
//
// Build and regenerate the test data with ./generate.sh (needs g++).
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <vector>

typedef int16_t int16;

typedef struct Parameters
{
	int16 saturation;
	int16 liveliness;
	int16 gamma;
	int16 contrast;
	int16 warmth;
	int16 hueshift;
} Parameters;

typedef struct TempParameters
{
	float Saturation = 0;
	float Liveliness = 0;
	float Gamma1 = 0;
	float Gamma2 = 0;
	float Contrast = 0;
	float Warmth = 0;
	float HueShift = 0;
} TempParameters;

typedef struct ColorRGBA
{
	float r, g, b, a;
} ColorRGBA;

static Parameters gParamsStorage;
static Parameters* gParams = &gParamsStorage;

// ---- verbatim from Dissolve.cpp ------------------------------------------------------------
// Pre-calculate as much as possible of what is being done in ProcessPixel
TempParameters PrecalcParameters() noexcept
{
	// Pre-calc values for ProcessPixel
	TempParameters temp = { 0 };
	temp.Saturation = gParams->saturation * gParams->saturation / 30000.0f;
	temp.Liveliness = powf((200 - gParams->liveliness) / 200.0f, 2);
	temp.Gamma1 = (200.0f - gParams->gamma) / 400.0f;
	temp.Gamma2 = (400.0f - gParams->gamma) / 800.0f;
	temp.Gamma1 = temp.Gamma1 * temp.Gamma1 * 4.0f;
	temp.Gamma2 = temp.Gamma2 * temp.Gamma2 * 4.0f;
	temp.Contrast = gParams->contrast / 100.0f;
	temp.Warmth = (gParams->warmth + 100.0f) / 200.0f;
	temp.Warmth = ((temp.Warmth * temp.Warmth) + 0.25f) * 2.0f;
	temp.HueShift = gParams->hueshift * 3.141593f / 180.0f;
	return temp;
}


// Called once per pixel
ColorRGBA ProcessPixel(ColorRGBA in, TempParameters temp) noexcept
{
	float val = (in.r + in.g + in.b) / 3.0f;
	in.r = in.r - val;
	in.g = in.g - val;
	in.b = in.b - val;
	const float difrg = sqrtf(in.r * in.r + in.g * in.g);
	const float difrb = sqrtf(in.r * in.r + in.b * in.b);
	const float difgb = sqrtf(in.g * in.g + in.b * in.b);
	float sat = (difrg + difrb + difgb) * temp.Saturation;

	sat = powf(sat, temp.Liveliness);
	val = powf(val, temp.Gamma1);
	sat = powf(sat, temp.Gamma2);

	if (temp.Contrast < -0.01f || temp.Contrast > 0.01f)
	{
		const float contrast = -cosf(val * 3.141593f) * 0.5f + 0.5f;
		val = contrast * temp.Contrast + (1 - temp.Contrast) * val;
	}

	float mx = in.r;
	float my = 0;
	mx = mx + in.g * cosf(2.094395f);
	my = my + in.g * sinf(2.094395f);
	mx = mx + in.b * cosf(-2.094395f);
	my = my + in.b * sinf(-2.094395f);

	const float hue = atan2f(-my, -mx) + temp.HueShift;	// from 0 to 3.141593

	// conv.back from superHSV
	float nr = -cosf(hue);
	float ng = -cosf(hue - 2.094395f);
	float nb = -cosf(hue + 2.094395f);

	// find saturation
	const float gr = (nr + ng + nb) / 3.0f;
	nr = gr * (1.0f - sat) + nr * sat;
	ng = gr * (1.0f - sat) + ng * sat;
	nb = gr * (1.0f - sat) + nb * sat;

	nr = (nr + val);
	ng = (ng + val);
	nb = (nb + val);

	nr = powf(nr, 1.0f / temp.Warmth);
	nb = powf(nb, temp.Warmth);

	in.r = nr;
	in.g = ng;
	in.b = nb;

	return in;
}
// ---- end of verbatim code ------------------------------------------------------------------

// Writes little-endian f32 records of 12 values each:
//   saturation, liveliness, gamma, contrast, warmth, hueshift, in.r, in.g, in.b, out.r, out.g, out.b
// The outputs are ProcessPixel()'s raw floats (unclamped, NaN included).
int main(int argc, char** argv)
{
	if (argc != 2) { fprintf(stderr, "usage: %s OUT.bin\n", argv[0]); return 2; }
	FILE* f = fopen(argv[1], "wb");
	if (!f) { perror(argv[1]); return 1; }

	// Neutral, every slider alone at several positions (including the ends), then mixtures.
	std::vector<Parameters> sets;
	const Parameters neutral = { 100, 0, 0, 0, 0, 0 };
	sets.push_back(neutral);
	const int16 sat[] = { 0, 1, 37, 99, 101, 150, 200 };
	const int16 pm100[] = { -100, -63, -1, 1, 2, 25, 100 };
	const int16 hue[] = { -180, -120, -45, 1, 90, 179, 180 };
	for (int16 v : sat) { Parameters p = neutral; p.saturation = v; sets.push_back(p); }
	for (int16 v : pm100) { Parameters p = neutral; p.liveliness = v; sets.push_back(p); }
	for (int16 v : pm100) { Parameters p = neutral; p.gamma = v; sets.push_back(p); }
	for (int16 v : pm100) { Parameters p = neutral; p.contrast = v; sets.push_back(p); }
	for (int16 v : pm100) { Parameters p = neutral; p.warmth = v; sets.push_back(p); }
	for (int16 v : hue) { Parameters p = neutral; p.hueshift = v; sets.push_back(p); }
	const Parameters mixes[] = {
		{ 160, 40, 20, 30, 15, 10 },
		{ 60, -50, -30, -40, -20, -90 },
		{ 200, 100, 100, 100, 100, 180 },
		{ 0, -100, -100, -100, -100, -180 },
		{ 130, 70, -10, 55, -35, 33 },
		{ 175, -25, 45, -75, 60, -150 },
	};
	for (const Parameters& p : mixes) sets.push_back(p);

	// Inputs: a 9 x 9 x 9 grid of the RGB cube plus a few 8-bit-exact values.
	std::vector<ColorRGBA> colours;
	for (int r = 0; r <= 8; r++)
		for (int g = 0; g <= 8; g++)
			for (int b = 0; b <= 8; b++)
				colours.push_back({ r / 8.0f, g / 8.0f, b / 8.0f, 1 });
	const int extra[][3] = { {255, 128, 0}, {1, 2, 3}, {200, 10, 250}, {17, 17, 17}, {254, 255, 253}, {90, 160, 30} };
	for (auto& e : extra) colours.push_back({ e[0] / 255.0f, e[1] / 255.0f, e[2] / 255.0f, 1 });

	for (const Parameters& p : sets)
	{
		*gParams = p;
		const TempParameters temp = PrecalcParameters();
		for (const ColorRGBA& in : colours)
		{
			const ColorRGBA out = ProcessPixel(in, temp);
			const float rec[12] = { (float)p.saturation, (float)p.liveliness, (float)p.gamma, (float)p.contrast,
				(float)p.warmth, (float)p.hueshift, in.r, in.g, in.b, out.r, out.g, out.b };
			fwrite(rec, sizeof rec, 1, f);
		}
	}
	fclose(f);
	fprintf(stderr, "%zu parameter sets x %zu colours\n", sets.size(), colours.size());
	return 0;
}
