#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "sam.h"
#include "reciter.h"
#include "render.h"

int main(int argc, char *argv[])
{
    if (argc < 2)
    {
        fprintf(stderr, "Usage: %s <text>\n", argv[0]);
        return 1;
    }

    SetInput(argv[1]);
    if (TextToPhonemes() == 0)
    {
        fprintf(stderr, "TextToPhonemes failed\n");
        return 1;
    }

    if (SAMMain() == 0)
    {
        fprintf(stderr, "SAMMain failed\n");
        return 1;
    }

    fwrite(GetBuffer(), 1, GetBufferLength(), stdout);
    return 0;
}
