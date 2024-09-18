#include <stdio.h>
#include <unistd.h>


int main_fut()
{
    printf("Hello, %c app!\n", 'C');
    usleep(100000);
    printf("Hello, %c app, sd!\n", 'C');

    return 0;
}

int async_main = 34;
