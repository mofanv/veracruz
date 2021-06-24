# This makefile is used within the docker image generated by docker/Dockerfile
#
# AUTHORS
#
# The Veracruz Development Team.
#
# COPYRIGHT
#
# See the `LICENSE.markdown` file in the Veracruz root directory for licensing
# and copyright information.
 
.PHONY: all sdk test_cases sgx-veracruz-client-test trustzone-veracruz-client-test sgx trustzone sgx-veracruz-server-test sgx-veracruz-server-performance sgx-veracruz-test sgx-psa-attestation tz-psa-attestationtrustzone-veracruz-server-test-setting  trustzone-veracruz-test-setting trustzone-env sgx-env trustzone-test-env clean clean-cargo-lock fmt 

 
WARNING_COLOR := "\e[1;33m"
INFO_COLOR := "\e[1;32m"
RESET_COLOR := "\e[0m"
OPTEE_DIR_SDK ?= /work/rust-optee-trustzone-sdk/
AARCH64_GCC ?= $(OPTEE_DIR)/toolchains/aarch64/bin/aarch64-linux-gnu-gcc
SGX_RUST_FLAG ?= "-L/work/sgxsdk/lib64 -L/work/sgxsdk/sdk_libs"
OPENSSL_INCLUDE_DIR ?= /usr/include/aarch64-linux-gnu
OPENSSL_LIB_DIR ?= /usr/lib/aarch64-linux-gnu
TRUSTZONE_C_INCLUDE_PATH ?= /usr/include/aarch64-linux-gnu:/usr/include
NITRO_RUST_FLAG ?= ""
 
all:
	@echo $(WARNING_COLOR)"Please explicitly choose a target."$(RESET_COLOR)

# Build all of the SDK and examples
sdk:
	$(MAKE) -C sdk

# Generate all test policy
sgx-test-collateral:
	TEE=sgx $(MAKE) -C test-collateral

# Test veracruz-client for sgx, due to the use of a mocked server with a fixed port, these tests must run in a single thread
sgx-veracruz-client-test: sgx sgx-test-collateral 
	cd veracruz-client && RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --lib --features "mock sgx" -- --test-threads=1

# Test veracruz-client for sgx, due to the use of a mocked server with a fixed port, these tests must run in a single thread
trustzone-veracruz-client-test: trustzone trustzone-test-collateral
	cd veracruz-client && cargo test --lib --features "mock tz" -- --test-threads=1

# Compile for sgx
# offset the CC OPENSSL_DIR, which might be used in compiling trustzone
sgx: sdk sgx-env
	cd runtime-manager-bind && RUSTFLAGS=$(SGX_RUST_FLAG) cargo build
	cd sgx-root-enclave-bind && RUSTFLAGS=$(SGX_RUST_FLAG) cargo build
	cd veracruz-client && RUSTFLAGS=$(SGX_RUST_FLAG) cargo build --lib --features sgx

nitro: sdk
	pwd
	RUSTFLAGS=$(NITRO_RUST_FLAG) $(MAKE) -C runtime-manager nitro
	RUSTFLAGS=$(NITRO_RUST_FLAG) $(MAKE) -C nitro-root-enclave
	RUSTFLAGS=$(NITRO_RUST_FLAG) $(MAKE) -C nitro-root-enclave-server

# Compile for trustzone, note: source the rust-optee-trustzone-sdk/environment first, however assume `unset CC`.
trustzone: sdk trustzone-env
	$(MAKE) -C runtime-manager trustzone CC=$(AARCH64_GCC) OPTEE_DIR=$(OPTEE_DIR) OPTEE_OS_DIR=$(OPTEE_OS_DIR)
	$(MAKE) -C trustzone-root-enclave trustzone OPTEE_DIR=$(OPTEE_DIR) OPTEE_OS_DIR=$(OPTEE_OS_DIR)
	cd veracruz-client && RUSTFLAGS=$(SGX_RUST_FLAG) cargo build --lib --features tz

# Using wildcard in the dependencies because if they are there, and newer, it
# should be rebuilt, but if they aren't there, they don't need to be built 
# (they are optional)
veracruz-test/proxy-attestation-server.db: $(wildcard sgx-root-enclave/css.bin) $(wildcard nitro-root-enclave/PCR0)
	cd veracruz-test &&
		bash ../test-collateral/populate-test-database.sh

# Using wildcard in the dependencies because if they are there, and newer, it
# should be rebuilt, but if they aren't there, they don't need to be built 
# (they are optional)
veracruz-server-test/proxy-attestation-server.db: $(wildcard sgx-root-enclave/css.bin) $(wildcard nitro-root-enclave/PCR0)
	cd veracruz-server-test && \
		bash ../test-collateral/populate-test-database.sh

sgx-veracruz-server-test: sgx test_cases veracruz-server-test/proxy-attestation-server.db
	cd veracruz-server-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --features sgx \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test test_debug --features sgx  -- --ignored --test-threads=1

sgx-veracruz-server-test-dry-run: sgx-test-collateral
	cd veracruz-server-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --features sgx --no-run 

sgx-veracruz-server-performance: sgx-test-collateral sgx test_cases veracruz-server-test/proxy-attestation-server.db
	cd veracruz-server-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test test_performance_ --features sgx -- --ignored 

sgx-veracruz-test-dry-run: sgx-test-collateral
	cd veracruz-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --features sgx --no-run

sgx-veracruz-test: sgx-test-collateral sgx test_cases veracruz-test/proxy-attestation-server.db
	cd veracruz-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --features sgx 

sgx-psa-attestation: sgx-env
	cd psa-attestation && cargo build --features sgx

tz-psa-attestation: trustzone-env
	cd psa-attestation && cargo build --target aarch64-unknown-linux-gnu --features tz

trustzone-veracruz-server-test: trustzone-test-collateral trustzone test_cases trustzone-test-env veracruz-server-test/proxy-attestation-server.db
	cd veracruz-server-test \
        && CC_aarch64_unknown_linux_gnu=$(AARCH64_GCC) OPENSSL_INCLUDE_DIR=$(OPENSSL_INCLUDE_DIR) \
                OPENSSL_LIB_DIR=$(OPENSSL_LIB_DIR) C_INCLUDE_PATH=$(TRUSTZONE_C_INCLUDE_PATH) \
                cargo test --target aarch64-unknown-linux-gnu --no-run --features tz -- --test-threads=1 \
		&& ./cp-veracruz-server-test-tz.sh
	chmod u+x run_veracruz_server_test_tz.sh
	./run_veracruz_server_test_tz.sh

trustzone-veracruz-test: trustzone-test-collateral trustzone test_cases trustzone-test-env veracruz-test/proxy-attestation-server.db
	cd veracruz-test \
        && CC_aarch64_unknown_linux_gnu=$(AARCH64_GCC) OPENSSL_INCLUDE_DIR=$(OPENSSL_INCLUDE_DIR) \
                OPENSSL_LIB_DIR=$(OPENSSL_LIB_DIR) C_INCLUDE_PATH=$(TRUSTZONE_C_INCLUDE_PATH) \
                cargo test --target aarch64-unknown-linux-gnu --no-run --features tz -- --test-threads=1
	cd veracruz-test && ./cp-veracruz-tz.sh
	chmod u+x run_veracruz_test_tz.sh
	./run_veracruz_test_tz.sh

trustzone-test-env: tz_test.sh run_tz_test.sh
	chmod u+x $^

nitro-veracruz-server-test: nitro test_cases veracruz-server-test/proxy-attestation-server.db
	cd veracruz-server-test \
		&& RUSTFLAGS=$(NITRO_RUST_FLAG) cargo test --features nitro \
		&& RUSTFLAGS=$(NITRO_RUST_FLAG) cargo test test_debug --features nitro,debug -- --ignored --test-threads=1
	cd veracruz-server-test \
		&& ./nitro-terminate.sh
	cd ./veracruz-server-test \
		&& ./nitro-ec2-terminate_root.sh

nitro-veracruz-server-test-dry-run: nitro test_cases
	cd veracruz-server-test \
		&& RUSTFLAGS=$(NITRO_RUST_FLAG) cargo test --features sgx --no-run

nitro-veracruz-server-performance: nitro test_cases veracruz-server-test/proxy-attestation-server.db
	cd veracruz-server-test \
		&& RUSTFLAGS=$(NITRO_RUST_FLAG) cargo test test_performance_ --features nitro -- --ignored
	cd veracruz-server-test \
		&& ./nitro-terminate.sh
	cd ./veracruz-server-test \
		&& ./nitro-ec2-terminate-root.sh

nitro-veracruz-test-dry-run: nitro test_cases
	cd veracruz-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --features nitro --no-run

nitro-veracruz-test: nitro test_cases  veracruz-test/proxy-attestation-server.db
	cd veracruz-test \
		&& RUSTFLAGS=$(SGX_RUST_FLAG) cargo test --features nitro
	cd veracruz-server-test \
		&& ./nitro-terminate.sh
	cd ./veracruz-server-test \
		&& ./nitro-ec2-terminate_root.sh

nitro-psa-attestation:
	cd psa-attestation && cargo build --features nitro

trustzone-env:
	unset CC
	rustup target add aarch64-unknown-linux-gnu arm-unknown-linux-gnueabihf
	rustup component add rust-src
	chmod u+x tz_test.sh

sgx-env:
	unset CC

clean:
	cd runtime-manager-bind && cargo clean 
	cd sgx-root-enclave-bind && cargo clean
	cd psa-attestation && cargo clean
	cd proxy-attestation-server && cargo clean
	cd session-manager && cargo clean
	cd veracruz-utils && cargo clean
	cd veracruz-server-test && cargo clean
	cd veracruz-test && cargo clean && rm -f proxy-attestation-server.db
	cd nitro-root-enclave-server && cargo clean
	$(MAKE) clean -C runtime-manager
	$(MAKE) clean -C sgx-root-enclave
	$(MAKE) clean -C veracruz-server
	$(MAKE) clean -C test-collateral 
	$(MAKE) clean -C trustzone-root-enclave
	$(MAKE) clean -C sdk
	$(MAKE) clean -C nitro-root-enclave

# NOTE: this target deletes ALL cargo.lock.
clean-cargo-lock:
	$(MAKE) clean -C sdk
	rm -f $(addsuffix /Cargo.lock,session-manager execution-engine transport-protocol veracruz-client sgx-root-enclave runtime-manager-bind runtime-manager psa-attestation veracruz-server-test veracruz-server sgx-root-enclave-bind trustzone-root-enclave proxy-attestation-server veracruz-test veracruz-util)

fmt:
	cd session-manager && cargo fmt
	cd execution-engine && cargo fmt
	cd transport-protocol && cargo fmt
	cd veracruz-client && cargo fmt
	cd sgx-root-enclave && cargo fmt
	cd runtime-manager && cargo fmt
	cd psa-attestation && cargo fmt
	cd veracruz-server-test && cargo fmt
	cd veracruz-server && cargo fmt
	cd veracruz-test && cargo fmt
	cd veracruz-utils && cargo fmt
	cd trustzone-root-enclave && cargo fmt
	cd proxy-attestation-server && cargo fmt
	$(MAKE) -C sdk fmt
