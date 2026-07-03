#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>
#include <string.h>
#include <stdlib.h>

#include "lib.h"

void WriteWav(char *filename, char *buffer, int bufferlength)
{
    FILE *file = fopen(filename, "wb");
    if (!file) return;

    // RIFF header
    fwrite("RIFF", 4, 1, file);
    unsigned int fileSize = 36 + bufferlength;
    fwrite(&fileSize, 4, 1, file);
    fwrite("WAVE", 4, 1, file);

    // fmt chunk
    fwrite("fmt ", 4, 1, file);
    unsigned int fmtSize = 16;
    fwrite(&fmtSize, 4, 1, file);
    unsigned short audioFormat = 1;
    fwrite(&audioFormat, 2, 1, file);
    unsigned short numChannels = 1;
    fwrite(&numChannels, 2, 1, file);
    unsigned int sampleRate = 22050;
    fwrite(&sampleRate, 4, 1, file);
    unsigned int byteRate = 22050;
    fwrite(&byteRate, 4, 1, file);
    unsigned short blockAlign = 1;
    fwrite(&blockAlign, 2, 1, file);
    unsigned short bitsPerSample = 8;
    fwrite(&bitsPerSample, 2, 1, file);

    // data chunk
    fwrite("data", 4, 1, file);
    unsigned int dataSize = bufferlength;
    fwrite(&dataSize, 4, 1, file);
    fwrite(buffer, 1, bufferlength, file);

    fclose(file);
}

int main(int argc, char *argv[])
{
    char *text;
    int i;

    if (argc < 2)
    {
        fprintf(stderr, "Usage: %s <text>\n", argv[0]);
        return 1;
    }

    text = argv[1];
    for (i = 0; text[i]; i++)
        text[i] = (char)toupper((unsigned char)text[i]);

    if (!SAMMain(text))
    {
        fprintf(stderr, "SAM synthesis failed\n");
        return 1;
    }

    char *buffer = GetBuffer();
    int buflen = GetBufferLength();
    if (buflen > 0)
        WriteWav("output.wav", buffer, buflen);

    return 0;
}
