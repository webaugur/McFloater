#include <stdio.h>
#include "debug.h"

int sam_debug = 0;

void PrintPhonemes(unsigned char *phonemeindex, unsigned char *phonemeLength, unsigned char *stress)
{
    int i = 0;
    fprintf(stderr,"===========================================\n");

    fprintf(stderr,"Internal Phoneme presentation:\n\n");
    fprintf(stderr," idx    phoneme  length  stress\n");
    fprintf(stderr,"------------------------------\n");

    while((phonemeindex[i] != 255) && (i < 255))
    {
        fprintf(stderr," %3d       %3d      %3d     %3d\n",
            i,
            phonemeindex[i],
            phonemeLength[i],
            stress[i]);
        i++;
    }
    fprintf(stderr,"\n");
}

void PrintOutput(unsigned char *phonemeindex, unsigned char *phonemeLength, unsigned char *stress)
{
    int i = 0;
    fprintf(stderr,"===========================================\n");

    fprintf(stderr,"Phoneme string:\n");
    while((phonemeindex[i] != 255) && (i < 255))
    {
        fprintf(stderr,"%c", phonemeindex[i]);
        i++;
    }
    fprintf(stderr,"\n");
}
