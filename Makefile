 .PHONY: build install clean

build:
	cargo build --release

install: build
	mkdir -p ~/.local/bin
	cp target/release/git-wt ~/.local/bin/
	chmod +x ~/.local/bin/git-wt
	echo "Installed git-wt to ~/.local/bin/git-wt"

clean:
	cargo clean