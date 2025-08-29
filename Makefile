check-contract:
	cosmwasm-check ./target/wasm32-unknown-unknown/release/lockup_contract.wasm

build:
	cargo wasm

build_tests:
	cargo build --tests

clean:
	cargo clean

optimize:
	@echo "!!!!!!! NOTE: for production use only intel porcessor, so no Mac M1 - see https://github.com/CosmWasm/optimizer"
# CosmWasm Rust Optimizer
	docker run  --rm -v .:/code \
	--mount type=volume,source="empty-contract_cache",target=/target \
	--mount type=volume,source=registry_cache,target=/usr/local/cargo/registry \
	cosmwasm/optimizer:0.16.0


# --user $$(id -u):$$(id -g)
# docker volume rm empty-contract_cache
# docker volume rm registry_cache

DOCKER_DIR := ./.docker
DOCKER_FILE := $(DOCKER_DIR)/Dockerfile

# Task to check if the file exists, and if not, do something

build-c4e-chain-docker:
	@if [ ! -f $(DOCKER_FILE) ]; then \
		echo "File $(DOCKER_FILE) does not exist, cloning repository..."; \
		-rm -rf $(DOCKER_DIR); \
		mkdir -p $(DOCKER_DIR); \
		git clone --branch v1.4.3.0 --depth 1 git@gitlab.sce.internal.ovoo.pl:c4e/chain/test/e2e-contract-test/c4e-chain-e2e-test-docker.git ./.docker; \
	else \
		echo "File $(DOCKER_FILE) already exists."; \
	fi
	docker build -t c4e-chain-e2e-test:v1.4.3 ./.docker

clean-dockerfile:
	-rm -rf $(DOCKER_DIR)

E2E_TEST_RUN_PATH=.e2e
E2E_TEST_CONFIG_PATH=./e2e-test/config

prepare_chain:
	@echo "------------- preparing: $(CHAIN) -------------"
	mkdir -p ${E2E_TEST_RUN_PATH}

	cp -r ${E2E_TEST_CONFIG_PATH}/common ${E2E_TEST_RUN_PATH}/node1
	cp -r ${E2E_TEST_CONFIG_PATH}/common ${E2E_TEST_RUN_PATH}/node2
	cp -r ${E2E_TEST_CONFIG_PATH}/common ${E2E_TEST_RUN_PATH}/node3
	cp -r ${E2E_TEST_CONFIG_PATH}/common ${E2E_TEST_RUN_PATH}/node4

	cp ${E2E_TEST_CONFIG_PATH}/node1/config/* ${E2E_TEST_RUN_PATH}/node1/config
	cp ${E2E_TEST_CONFIG_PATH}/node2/config/* ${E2E_TEST_RUN_PATH}/node2/config
	cp ${E2E_TEST_CONFIG_PATH}/node3/config/* ${E2E_TEST_RUN_PATH}/node3/config
	cp ${E2E_TEST_CONFIG_PATH}/node4/config/* ${E2E_TEST_RUN_PATH}/node4/config

	$(MAKE) _replace REPLACE_FILE=${E2E_TEST_CONFIG_PATH}/replace.cfg


clean_prepare_chain:
	-rm -r ${E2E_TEST_RUN_PATH}

DOCKER_GROUP=did-contract

run_chain:
	-docker network create did
	docker run --name chain-node-did-1 -d --user $$(id -u):$$(id -g) -v ./${E2E_TEST_RUN_PATH}/node1/:/chain4energy/.c4e-chain/ --network did --label com.docker.compose.project=${DOCKER_GROUP} -p 31657:26657 --rm c4e-chain-did:v1.4.3
	docker run --name chain-node-did-2 -d --user $$(id -u):$$(id -g) -v ./${E2E_TEST_RUN_PATH}/node2/:/chain4energy/.c4e-chain/ --network did --label com.docker.compose.project=${DOCKER_GROUP} --rm c4e-chain-did:v1.4.3
	docker run --name chain-node-did-3 -d --user $$(id -u):$$(id -g) -v ./${E2E_TEST_RUN_PATH}/node3/:/chain4energy/.c4e-chain/ --network did --label com.docker.compose.project=${DOCKER_GROUP} --rm c4e-chain-did:v1.4.3
	docker run --name chain-node-did-4 -d --user $$(id -u):$$(id -g) -v ./${E2E_TEST_RUN_PATH}/node4/:/chain4energy/.c4e-chain/ --network did --label com.docker.compose.project=${DOCKER_GROUP} --rm c4e-chain-did:v1.4.3

stop_chain:
	@echo "Stopping all containers with label com.docker.compose.project=${DOCKER_GROUP}"
	docker ps -q --filter "label=com.docker.compose.project=${DOCKER_GROUP}" | xargs -r docker stop
	@echo "Removing all containers with label com.docker.compose.project=${DOCKER_GROUP}"
	docker ps -a -q --filter "label=com.docker.compose.project=${DOCKER_GROUP}" | xargs -r docker rm
	@echo "Removing the did network"
	-docker network rm did

_replace:
	@echo "Replacing according to ${REPLACE_FILE}"
	@bash -c ' \
	while IFS="," read -r file old_value new_value; do \
		if [ -z "$$file" ] || [ -z "$$old_value" ] || [ -z "$$new_value" ]; then \
			echo "Skipping line due to missing parameters: file=$$file, old_value=$$old_value, new_value=$$new_value"; \
			continue; \
		fi; \
		echo "raplacing $$old_value to $$new_value in file $$file"; \
		sed -i "s/$$old_value/$$new_value/g" ${E2E_TEST_RUN_PATH}/$$file; \
	done < ${REPLACE_FILE}'

expand:
	-mkdir .expand
	cargo expand > .expand/expand.rs

update_git_dependencies:
	cargo update
# cargo update -p docker-controller
# cargo update -p cosm-client
# cargo update -p e2e-test-suite
