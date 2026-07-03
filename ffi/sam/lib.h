#ifndef LIB_H
#define LIB_H

struct speak_result {
    int res;
    unsigned char *buf;
    int buf_size;
};

struct speak_result speakText(char *text);
void setupSpeak(unsigned char pitch, unsigned char speed, unsigned char throat, unsigned char mouth);

#endif
