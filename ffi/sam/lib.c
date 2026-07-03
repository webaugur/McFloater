#include <stdlib.h>
#include <string.h>
#include "lib.h"
#include "sam.h"
#include "reciter.h"
#include "render.h"

struct speak_result speakText(char *text)
{
    struct speak_result result;
    result.res = 0;
    result.buf = NULL;
    result.buf_size = 0;

    if (text == NULL) return result;

    SetInput(text);
    if (TextToPhonemes() == 0) return result;

    if (SAMMain() == 0) return result;

    result.buf = (unsigned char*)GetBuffer();
    result.buf_size = GetBufferLength();
    result.res = 1;
    return result;
}

void setupSpeak(unsigned char pitch, unsigned char speed, unsigned char throat, unsigned char mouth)
{
    SetPitch(pitch);
    SetSpeed(speed);
    SetThroat(throat);
    SetMouth(mouth);
}
