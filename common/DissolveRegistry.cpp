//-------------------------------------------------------------------------------

#include "DissolveRegistry.h"

//-------------------------------------------------------------------------------
//
// ReadRegistryParameters
//
// Read our parameters out of the Photoshop Registry. This saves us from writing
// to a preferences file and the pains of managing that on each OS. You should 
// probably ignore errors from this routine as other hosts have not 
// implemented a registry.
//
//-------------------------------------------------------------------------------
SPErr ReadRegistryParameters(void)
{
	SPErr err = noErr;
	SPBasicSuite* basicSuite = gFilterRecord->sSPBasic;
	PSDescriptorRegistryProcs* registryProcs = NULL;
	PSActionDescriptorProcs* descriptorProcs = NULL;
	PIActionDescriptor descriptor = NULL;
	DescriptorUnitID unit;
	DescriptorEnumTypeID type;
	DescriptorEnumID disposition;
	Boolean ignoreSelection;
	double tempValue;

	if (basicSuite == NULL)		return errPlugInHostInsufficient;

	err = basicSuite->AcquireSuite(kPSDescriptorRegistrySuite, kPSDescriptorRegistrySuiteVersion, (const void**)&registryProcs);
	if (err) goto returnError;

	err = basicSuite->AcquireSuite(kPSActionDescriptorSuite, kPSActionDescriptorSuiteVersion, (const void**)&descriptorProcs);
	if (err) goto returnError;

	err = registryProcs->Get(plugInUniqueID, &descriptor);
	if (err) goto returnError;
	if (descriptor == NULL) goto returnError;

	err = descriptorProcs->GetUnitFloat(descriptor, keySaturation, &unit, &tempValue);
	if (err) goto returnError;
	gParams->saturation = (int16)tempValue;

	err = descriptorProcs->GetUnitFloat(descriptor, keyLiveliness, &unit, &tempValue);
	if (err) goto returnError;
	gParams->liveliness = (int16)tempValue;

	err = descriptorProcs->GetUnitFloat(descriptor, keyGamma, &unit, &tempValue);
	if (err) goto returnError;
	gParams->gamma = (int16)tempValue;

	err = descriptorProcs->GetUnitFloat(descriptor, keyContrast, &unit, &tempValue);
	if (err) goto returnError;
	gParams->contrast = (int16)tempValue;

	err = descriptorProcs->GetUnitFloat(descriptor, keyWarmth, &unit, &tempValue);
	if (err) goto returnError;
	gParams->warmth = (int16)tempValue;

	err = descriptorProcs->GetUnitFloat(descriptor, keyHue, &unit, &tempValue);
	if (err) goto returnError;
	gParams->hueshift = (int16)tempValue;

	err = descriptorProcs->GetUnitFloat(descriptor, keyAmount, &unit, &tempValue);
	if (err) goto returnError;
	gParams->percent = (int16)tempValue;

	err = descriptorProcs->GetEnumerated(descriptor, keyDisposition, &type, &disposition);
	if (err) goto returnError;
	gParams->disposition = ScriptToDialog(disposition);
	CopyColor(gData->color, gData->colorArray[gParams->disposition]);

	err = descriptorProcs->GetBoolean(descriptor, keyIgnoreSelection, &ignoreSelection);
	if (err) goto returnError;
	gParams->ignoreSelection = ignoreSelection;

returnError:
	if (descriptor != NULL)			descriptorProcs->Free(descriptor);
	if (registryProcs != NULL)		basicSuite->ReleaseSuite(kPSDescriptorRegistrySuite, kPSDescriptorRegistrySuiteVersion);
	if (descriptorProcs != NULL)	basicSuite->ReleaseSuite(kPSActionDescriptorSuite, kPSActionDescriptorSuiteVersion);
	return err;
}



//-------------------------------------------------------------------------------
//
// WriteRegistryParameters
//
// Write our parameters out to the Photoshop Registry. This saves us from writing
// to a preferences file and the pains of managing that on each OS. You should 
// probably ignore errors from this routine as other hosts have not 
// implemented a registry.
// 
//-------------------------------------------------------------------------------
SPErr WriteRegistryParameters(void)
{
	SPErr err = noErr;
	SPBasicSuite* basicSuite = gFilterRecord->sSPBasic;
	PSDescriptorRegistryProcs* registryProcs = NULL;
	PSActionDescriptorProcs* descriptorProcs = NULL;
	PIActionDescriptor descriptor = NULL;


	if (basicSuite == NULL)		return errPlugInHostInsufficient;

	err = basicSuite->AcquireSuite(kPSDescriptorRegistrySuite, kPSDescriptorRegistrySuiteVersion, (const void**)&registryProcs);
	if (err) goto returnError;

	err = basicSuite->AcquireSuite(kPSActionDescriptorSuite, kPSActionDescriptorSuiteVersion, (const void**)&descriptorProcs);
	if (err) goto returnError;

	err = descriptorProcs->Make(&descriptor);
	if (err) goto returnError;

	err = descriptorProcs->PutUnitFloat(descriptor, keySaturation, unitPercent, (int32)gParams->saturation);	if (err) goto returnError;
	err = descriptorProcs->PutUnitFloat(descriptor, keyLiveliness, unitPercent, (int32)gParams->liveliness);	if (err) goto returnError;
	err = descriptorProcs->PutUnitFloat(descriptor, keyGamma, unitPercent, (int32)gParams->gamma);				if (err) goto returnError;
	err = descriptorProcs->PutUnitFloat(descriptor, keyContrast, unitPercent, (int32)gParams->contrast);		if (err) goto returnError;
	err = descriptorProcs->PutUnitFloat(descriptor, keyWarmth, unitPercent, (int32)gParams->warmth);			if (err) goto returnError;
	err = descriptorProcs->PutUnitFloat(descriptor, keyHue, unitPercent, (int32)gParams->hueshift);		if (err) goto returnError;
	err = descriptorProcs->PutUnitFloat(descriptor, keyAmount, unitPercent, (int32)gParams->percent);			if (err) goto returnError;

	err = descriptorProcs->PutEnumerated(descriptor, keyDisposition, typeMood, DialogToScript(gParams->disposition));
	if (err) goto returnError;

	err = descriptorProcs->PutBoolean(descriptor, keyIgnoreSelection, gParams->ignoreSelection);
	if (err) goto returnError;

	err = registryProcs->Register(plugInUniqueID, descriptor, true);
	if (err) goto returnError;

returnError:
	if (descriptor != NULL)			descriptorProcs->Free(descriptor);
	if (registryProcs != NULL)		basicSuite->ReleaseSuite(kPSDescriptorRegistrySuite, kPSDescriptorRegistrySuiteVersion);
	if (descriptorProcs != NULL)	basicSuite->ReleaseSuite(kPSActionDescriptorSuite, kPSActionDescriptorSuiteVersion);
	return err;
}
