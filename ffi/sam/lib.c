#include <ctype.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>

#include "reciter.h"
#include "sam.h"
#include "lib.h"

int sam_debug = 0;

void setupSpeak(unsigned char pitch,unsigned char speed,unsigned char throat,unsigned char mouth) {
    SetPitch(pitch == 0 ? 64 : pitch);
    SetSpeed(speed == 0 ? 72 : speed);
    SetThroat(throat == 0 ? 128 :throat);
    SetMouth(mouth == 0 ? 128 : mouth);
}

struct AudioResult* speakText(char *input)
{
    char phoneme_input[256];
    int i;

    memset(phoneme_input, 0, sizeof(phoneme_input));
    strncpy(phoneme_input, input, sizeof(phoneme_input) - 2);

    for (i = 0; phoneme_input[i] != 0; i++) {
        phoneme_input[i] = (char)toupper((unsigned char)phoneme_input[i]);
    }
    strncat(phoneme_input, "[", sizeof(phoneme_input) - strlen(phoneme_input) - 1);

    TextToPhonemes((unsigned char*) phoneme_input);
    if (sam_debug) {
        fprintf(stderr, "Phonemes: %s\n", phoneme_input);
    }
    SetInput(phoneme_input);
    struct AudioResult *resp = malloc(sizeof(struct AudioResult));
    resp -> res = SAMMain();
    resp -> buf = GetBuffer();
    resp -> buf_size = GetBufferLength() / 50;
    return resp;
}

