#include <stdio.h>
#include <string.h>
#include "reciter.h"
#include "ReciterTabs.h"
#include "debug.h"

unsigned char A, X, Y;
extern int sam_debug;

static unsigned char inputtemp[256];   // secure copy of input tab36096

void Code37055(unsigned char mem59)
{
    X = mem59;
    X--;
    A = inputtemp[X];
    Y = A;
    A = tab36376[Y];
    return;
}

void Code37066(unsigned char mem58)
{
    X = mem58;
    X++;
    A = inputtemp[X];
    Y = A;
    A = tab36376[Y];
}

unsigned char GetRuleByte(unsigned short mem62, unsigned char Y)
{
    unsigned int address = mem62;

    if (mem62 >= 37541)
    {
        address -= 37541;
        return rules2[address+Y];
    }
    address -= 32000;
    return rules[address+Y];
}

int TextToPhonemes(unsigned char *input) // Code36484
{
    unsigned char mem56;
    unsigned char mem57;
    unsigned char mem58;
    unsigned char mem59;
    unsigned char mem60;
    unsigned char mem61;
    unsigned short mem62;
    unsigned char mem64;
    unsigned char mem65;
    unsigned char mem66;
    unsigned char mem36653;

    inputtemp[0] = 32;
    X = 1;
    Y = 0;
    do
    {
        A = input[Y] & 127;
        if ( A >= 112) A = A & 95;
        else if ( A >= 96) A = A & 79;
        inputtemp[X] = A;
        X++;
        Y++;
    } while (Y != 255);

    X = 255;
    inputtemp[X] = 27;
    mem61 = 255;

pos36550:
    A = 255;
    mem56 = 255;

pos36554:
    while(1)
    {
        mem61++;
        X = mem61;
        A = inputtemp[X];
        mem64 = A;
        if (A == '[')
        {
            mem56++;
            X = mem56;
            A = 155;
            input[X] = 155;
            return 1;
        }
        if (A != '.') break;
        X++;
        Y = inputtemp[X];
        A = tab36376[Y] & 1;
        if(A != 0) break;
        mem56++;
        X = mem56;
        A = '.';
        input[X] = '.';
    }

    A = mem64;
    Y = A;
    A = tab36376[A];
    mem57 = A;
    if((A&2) != 0)
    {
        mem62 = 37541;
        goto pos36700;
    }

    A = mem57;
    if(A != 0) goto pos36677;
    A = 32;
    inputtemp[X] = ' ';
    mem56++;
    X = mem56;
    if (X > 120) goto pos36654;
    input[X] = A;
    goto pos36554;

pos36654:
    input[X] = 155;
    A = mem61;
    mem36653 = A;
    return 1;

pos36677:
    A = mem57 & 128;
    if(A == 0) return 0;
    X = mem64 - 'A';
    mem62 = tab37489[X] | (tab37515[X]<<8);

pos36700:
    Y = 0;
    do { mem62 += 1; A = GetRuleByte(mem62, Y); } while ((A & 128) == 0);
    Y++;
    while(1) { A = GetRuleByte(mem62, Y); if (A == '(') break; Y++; }
    mem66 = Y;
    do { Y++; A = GetRuleByte(mem62, Y); } while(A != ')');
    mem65 = Y;
    do { Y++; A = GetRuleByte(mem62, Y); A = A & 127; } while (A != '=');
    mem64 = Y;
    X = mem61; mem60 = X;
    Y = mem66; Y++;
    while(1)
    {
        mem57 = inputtemp[X];
        A = GetRuleByte(mem62, Y);
        if (A != mem57) goto pos36700;
        Y++;
        if(Y == mem65) break;
        X++; mem60 = X;
    }
    A = mem61; mem59 = mem61;
pos37455:
    Y = mem64; mem61 = mem60;
    if (sam_debug) PrintRule(mem62);
pos37461:
    A = GetRuleByte(mem62, Y);
    mem57 = A;
    A = A & 127;
    if (A != '=') { mem56++; X = mem56; input[X] = A; }
    if ((mem57 & 128) == 0) { Y++; goto pos37461; }
    goto pos36554;
}
