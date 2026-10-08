#include <fcntl.h>
#include <stddef.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    int fd = open(argv[1], O_RDWR | O_CLOEXEC);
    if (fd < 0) return 3;
    struct stat info;
    if (fstat(fd, &info) != 0 || info.st_size < 3) return 4;
    size_t length = (size_t)info.st_size;
    char *bytes = mmap(NULL, length, PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    close(fd);
    if (bytes == MAP_FAILED) return 5;
    int replaced = 0;
    for (size_t index = 0; index + 2 < length; ++index) {
        if (bytes[index] == '1' && bytes[index + 1] == ' ' && bytes[index + 2] == '+') {
            bytes[index] = '7';
            replaced = 1;
            break;
        }
    }
    if (!replaced || msync(bytes, length, MS_SYNC) != 0 || munmap(bytes, length) != 0) return 6;
    return 0;
}
