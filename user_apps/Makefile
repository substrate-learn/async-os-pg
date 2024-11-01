TARGET := riscv64gc-unknown-none-elf
MODE := debug
APP_DIR := src/bin
TARGET_DIR := ../target/$(TARGET)/$(MODE)
APPS := $(wildcard $(APP_DIR)/*.rs)
ELFS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%, $(APPS))
BINS := $(patsubst $(APP_DIR)/%.rs, $(TARGET_DIR)/%.bin, $(APPS))

CARGO_MODE_RELEASE := --release

OBJDUMP := rust-objdump --arch-name=riscv64
OBJCOPY := rust-objcopy --binary-architecture=riscv64
CP := cp

TEST ?=

elf: $(APPS)
	@cargo build $(CARGO_MODE_$(MODE))
ifeq ($(TEST), 1)
	@$(CP) $(TARGET_DIR)/usertests $(TARGET_DIR)/initproc
endif

binary: elf
	@$(foreach elf, $(ELFS), $(OBJCOPY) $(elf) --strip-all -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

binary_without_strip: elf
	@$(foreach elf, $(ELFS), $(OBJCOPY) $(elf) -O binary $(patsubst $(TARGET_DIR)/%, $(TARGET_DIR)/%.bin, $(elf));)

build: binary

build_without_strip: binary_without_strip

clean:
	@cargo clean

build_img:
	@cd .. && ./build_img.sh -a riscv64

remake_disk_img:
	@$(foreach elf_file, $(ELFS), cp $(elf_file) ../testcases/riscv64_linux_musl/)
	@cd .. && ./build_img.sh -a riscv64

my_process: build_without_strip remake_disk_img

.PHONY: elf binary build clean
