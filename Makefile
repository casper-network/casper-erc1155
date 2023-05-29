ALL_CONTRACTS = erc1155-token erc1155-test erc1155-test-call
CONTRACT_TARGET_DIR = target/wasm32-unknown-unknown/release

prepare:
	rustup target add wasm32-unknown-unknown

.PHONY:	build-contracts
build-contracts:
	cargo build --release --target wasm32-unknown-unknown $(patsubst %, -p %, $(ALL_CONTRACTS))
	$(foreach WASM, $(ALL_CONTRACTS), wasm-strip $(CONTRACT_TARGET_DIR)/$(subst -,_,$(WASM)).wasm 2>/dev/null | true;)
	cp target/wasm32-unknown-unknown/release/erc1155_token.wasm example/erc1155-tests/wasm/erc1155_token.wasm
	cp target/wasm32-unknown-unknown/release/erc1155_test.wasm testing/tests/wasm/erc1155_test.wasm
	cp target/wasm32-unknown-unknown/release/erc1155_test_call.wasm testing/tests/wasm/erc1155_test_call.wasm
	cp target/wasm32-unknown-unknown/release/erc1155_token.wasm testing/tests/wasm/erc1155_token.wasm

test: build-contracts
	cargo test

clippy:
	cargo clippy --all-targets -- -D warnings
	cargo clippy -p erc1155-token --target wasm32-unknown-unknown -- -D warnings

check-lint: clippy
	cargo fmt --all -- --check

lint: clippy
	cargo fmt --all

clean:
	cargo clean
