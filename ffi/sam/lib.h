#include "reciter.h"
#include "sam.h"

extern int sam_debug;

struct AudioResult {
    int res;
    int buf_size;
    char *buf;
};

void setupSpeak(unsigned char pitch,unsigned char speed,unsigned char throat,unsigned char mouth);

struct AudioResult* speakText(char *input);
