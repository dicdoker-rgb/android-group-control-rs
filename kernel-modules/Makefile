obj-m += mounts_filter_v2.o

KDIR ?= /root/linux-orangepi-orange-pi-5.10-rk35xx

all:
	make -C $(KDIR) M=$(PWD) modules ARCH=arm64

clean:
	make -C $(KDIR) M=$(PWD) clean
