{ self, ... }: {
  perSystem = { self', pkgs, systemCommonRust, subnix, lib, system, ... }:
    let
      devnet-root-directory = "/tmp/composable-devnet";
      validator-key = "osmo12smx2wdlyttvyzvzg54y2vnqwq2qjateuf7thj";
    in {
      packages = rec {
        osmosisd = pkgs.writeShellApplication {
          name = "osmosisd";
          text = ''
            ${self.inputs.cosmos.packages.${system}.osmosis}/bin/osmosisd "$@"
          '';
        };
        hermes = self.inputs.cosmos.packages.${system}.hermes_1_5_1;
        hermes-init = pkgs.writeShellApplication {
          runtimeInputs = [ hermes ];
          name = "hermes-init";
          text = ''
            HOME=/home/dz/github.com/ComposableFi/composable
            MNEMONIC_FILE=$HOME/.hermes/mnemonics/relayer.txt
            export HOME
            mkdir --parents $HOME/.hermes/mnemonics/

            echo "black frequent sponsor nice claim rally hunt suit parent size stumble expire forest avocado mistake agree trend witness lounge shiver image smoke stool chicken" > $MNEMONIC_FILE
            hermes keys add --chain centauri-dev --mnemonic-file $MNEMONIC_FILE --key-name centauri-dev --overwrite
            hermes keys add --chain osmosis-dev --mnemonic-file $MNEMONIC_FILE --key-name osmosis-dev --overwrite
            RUST_LOG=debug
            export RUST_LOG
            hermes create channel --a-chain centauri-dev --b-chain osmosis-dev --a-port transfer --b-port transfer --new-client-connection --yes
            hermes start
          '';
        };

        osmosisd-gen = pkgs.writeShellApplication {
          name = "osmosisd-gen";
          runtimeInputs = [ osmosisd pkgs.jq pkgs.yq pkgs.dasel ];
          text = ''
            HOME=${devnet-root-directory}
            export HOME
            OSMOSIS_DATA="$HOME/.osmosisd"             
            CHAIN_ID="osmosis-dev"
            REUSE=true
            export REUSE
            if [[ $REUSE == false ]]; then
              rm --force --recursive "$OSMOSIS_DATA" 
            fi

            VALIDATOR_MONIKER="validator"
            VALIDATOR_MNEMONIC="bottom loan skill merry east cradle onion journey palm apology verb edit desert impose absurd oil bubble sweet glove shallow size build burst effort"
            FAUCET_MNEMONIC="increase bread alpha rigid glide amused approve oblige print asset idea enact lawn proof unfold jeans rabbit audit return chuckle valve rather cactus great"
            RELAYER_MNEMONIC="black frequent sponsor nice claim rally hunt suit parent size stumble expire forest avocado mistake agree trend witness lounge shiver image smoke stool chicken"
            CONFIG_FOLDER=$OSMOSIS_DATA/config
            GENESIS=$CONFIG_FOLDER/genesis.json
            mkdir --parents "$OSMOSIS_DATA/data/cs.wal"

            echo "$VALIDATOR_MNEMONIC" | osmosisd init --chain-id="$CHAIN_ID" --home "$OSMOSIS_DATA" --recover "$VALIDATOR_MONIKER"

            function dasel-genesis() {
              dasel put --type string --file "$GENESIS" --value "$2" "$1"   
            }             

            dasel-genesis '.app_state.staking.params.bond_denom' 'uosmo'
            dasel-genesis '.app_state.staking.params.unbonding_time' '120s'
            dasel  put --type json --file "$GENESIS" --value "[{},{}]" 'app_state.bank.denom_metadata'
            dasel-genesis '.app_state.bank.denom_metadata.[0].description' 'Registered denom uion for localosmosis testing'
            dasel  put --type json --file "$GENESIS" --value "[{}]" '.app_state.bank.denom_metadata.[0].denom_units'
            dasel-genesis '.app_state.bank.denom_metadata.[0].denom_units.[0].denom' 'uion'
            dasel-genesis '.app_state.bank.denom_metadata.[0].denom_units.[0].exponent' 0
            dasel-genesis '.app_state.bank.denom_metadata.[0].base' 'uion'
            dasel-genesis '.app_state.bank.denom_metadata.[0].display' 'uion'
            dasel-genesis '.app_state.bank.denom_metadata.[0].name' 'uion'
            dasel-genesis '.app_state.bank.denom_metadata.[0].symbol' 'uion'

            dasel  put --type json --file "$GENESIS" --value "[{}]" '.app_state.bank.denom_metadata.[1].denom_units'
            dasel-genesis '.app_state.bank.denom_metadata.[1].description' 'Registered denom uosmo for localosmosis testing'
            dasel-genesis '.app_state.bank.denom_metadata.[1].denom_units.[0].denom' 'uosmo'
            dasel-genesis '.app_state.bank.denom_metadata.[1].denom_units.[0].exponent' 0
            dasel-genesis '.app_state.bank.denom_metadata.[1].base' 'uosmo'
            dasel-genesis '.app_state.bank.denom_metadata.[1].display' 'uosmo'
            dasel-genesis '.app_state.bank.denom_metadata.[1].name' 'uosmo'
            dasel-genesis '.app_state.bank.denom_metadata.[1].symbol' 'uosmo'

            dasel-genesis '.app_state.crisis.constant_fee.denom' 'uosmo'
            dasel-genesis '.app_state.gov.voting_params.voting_period' '30s'
            dasel  put --type json --file "$GENESIS" --value "[{}]" '.app_state.gov.deposit_params.min_deposit'
            dasel-genesis '.app_state.gov.deposit_params.min_deposit.[0].denom' 'uosmo'
            dasel-genesis '.app_state.gov.deposit_params.min_deposit.[0].amount' '1000000000'
            dasel-genesis '.app_state.epochs.epochs.[1].duration' "60s"
            dasel  put --type json --file "$GENESIS" --value "[{},{},{}]" '.app_state.poolincentives.lockable_durations'
            dasel-genesis '.app_state.poolincentives.lockable_durations.[0]' "120s"
            dasel-genesis '.app_state.poolincentives.lockable_durations.[1]' "180s"
            dasel-genesis '.app_state.poolincentives.lockable_durations.[2]' "240s"
            dasel-genesis '.app_state.poolincentives.params.minted_denom' "uosmo"
            dasel  put --type json --file "$GENESIS" --value "[{},{},{},{}]" '.app_state.incentives.lockable_durations'
            dasel-genesis '.app_state.incentives.lockable_durations.[0]' "1s"
            dasel-genesis '.app_state.incentives.lockable_durations.[1]' "120s"
            dasel-genesis '.app_state.incentives.lockable_durations.[2]' "180s"
            dasel-genesis '.app_state.incentives.lockable_durations.[3]' "240s"
            dasel-genesis '.app_state.incentives.params.distr_epoch_identifier' "hour"
            dasel-genesis '.app_state.mint.params.mint_denom' "uosmo"
            dasel-genesis '.app_state.mint.params.epoch_identifier' "day"
            dasel-genesis '.app_state.poolmanager.params.pool_creation_fee.[0].denom' "uosmo"

            dasel  put --type json --file "$GENESIS" --value "[{}]" '.app_state.gamm.params.pool_creation_fee'
            dasel-genesis '.app_state.gamm.params.pool_creation_fee.[0].denom' "uosmo"
            dasel-genesis '.app_state.gamm.params.pool_creation_fee.[0].amount' "10000000"
            dasel-genesis '.app_state.txfees.basedenom' "uosmo"
            dasel-genesis '.app_state.wasm.params.code_upload_access.permission' "Everybody"
            dasel-genesis '.app_state.concentratedliquidity.params.is_permissionless_pool_creation_enabled' true

            function add-genesis-account() {
              echo "$1" | osmosisd keys add "$2" --recover --keyring-backend test --home "$OSMOSIS_DATA" 
              ACCOUNT=$(osmosisd keys show --address "$2" --keyring-backend test --home "$OSMOSIS_DATA" )
              echo "===================================="
              echo "$ACCOUNT"
              osmosisd add-genesis-account "$ACCOUNT" 100000000000uosmo,100000000000uion,100000000000stake --home "$OSMOSIS_DATA"
            }

            add-genesis-account "$VALIDATOR_MNEMONIC" "$VALIDATOR_MONIKER"
            add-genesis-account "$FAUCET_MNEMONIC" "faucet"
            add-genesis-account "$RELAYER_MNEMONIC" "relayer"

            osmosisd gentx $VALIDATOR_MONIKER 500000000uosmo --keyring-backend=test --chain-id=$CHAIN_ID --home "$OSMOSIS_DATA" 
            osmosisd collect-gentxs --home "$OSMOSIS_DATA"
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value "" '.p2p.seeds'
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value "tcp://0.0.0.0:36657" '.rpc.laddr'
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value "0.0.0.0:16060" '.rpc.pprof_laddr'
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value "tcp://127.0.0.1:36658" '.proxy_app'
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value ":36660" '.instrumentation.prometheus_listen_addr'
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value "tcp://0.0.0.0:36656" '.p2p.laddr'
            dasel put --type string --file "$CONFIG_FOLDER/config.toml" --value "tcp://localhost:36657" '.node'

            dasel put --type string --file "$CONFIG_FOLDER/app.toml" --value "0.0.0.0:19090" '.grpc.address'
            dasel put --type string --file "$CONFIG_FOLDER/app.toml" --value "0.0.0.0:19091" '.grpc-web.address'
            dasel put --type string --file "$CONFIG_FOLDER/app.toml" --value "tcp://0.0.0.0:11317" '.api.address'

            dasel put --type string --file $CONFIG_FOLDER/config.toml --value "*" '.rpc.cors_allowed_origins.[]'
            dasel put --type string --file $CONFIG_FOLDER/config.toml --value "Accept-Encoding" '.rpc.cors_allowed_headers.[]'
            dasel put --type string --file $CONFIG_FOLDER/config.toml --value "DELETE" '.rpc.cors_allowed_methods.[]'
            dasel put --type string --file $CONFIG_FOLDER/config.toml --value "OPTIONS" '.rpc.cors_allowed_methods.[]'
            dasel put --type string --file $CONFIG_FOLDER/config.toml --value "PATCH" '.rpc.cors_allowed_methods.[]'
            dasel put --type string --file $CONFIG_FOLDER/config.toml --value "PUT" '.rpc.cors_allowed_methods.[]'
            dasel put --type bool --file $CONFIG_FOLDER/app.toml --value "true" '.api.swagger'
            dasel put --type bool --file $CONFIG_FOLDER/app.toml --value "true" '.api.enabled-unsafe-cors'
            dasel put --type bool --file $CONFIG_FOLDER/app.toml --value "true" '.grpc-web.enable-unsafe-cors'

            dasel put --type string --file $CONFIG_FOLDER/client.toml --value "tcp://localhost:36657" '.node'
            dasel put --type string --file $CONFIG_FOLDER/client.toml --value "$CHAIN_ID" '.chain-id'
            dasel put --type string --file $CONFIG_FOLDER/client.toml --value "test" '.keyring-backend'
            dasel put --type string --file $CONFIG_FOLDER/client.toml --value "json" '.output'


            osmosisd start --home "$OSMOSIS_DATA" --rpc.unsafe --rpc.laddr tcp://0.0.0.0:36657 --pruning=nothing --grpc.address localhost:19090   --address "tcp://0.0.0.0:36658" --p2p.external-address 43421 --p2p.laddr "tcp://0.0.0.0:36656" --p2p.pex false --p2p.upnp  false  --p2p.seed_mode true
          '';
        };

        osmosisd-init = pkgs.writeShellApplication {
          name = "osmosisd-init";
          runtimeInputs = [ osmosisd pkgs.jq pkgs.yq pkgs.dasel ];
          text = ''
            HOME=${devnet-root-directory}
            export HOME
            OSMOSIS_DATA="$HOME/.osmosisd"             
            CHAIN_ID="osmosis-dev"

            set +e
            osmosisd tx wasm store  "${self'.packages.xcvm-contracts}/lib/cw_xc_gateway.wasm" --chain-id="$CHAIN_ID"  --node "tcp://localhost:36657" --output json --yes --gas 25000000 --fees 920000166uatom --dry-run --log_level trace --trace --keyring-backend test  --home "$OSMOSIS_DATA" --from validator
          '';
        };
      };
      process-compose.devnet-xc = {
        settings = {
          processes = {
            centauri = {
              command = self'.packages.centaurid-gen;
              readiness_probe.http_get = {
                host = "127.0.0.1";
                port = 26657;
              };
            };
            centauri-init = {
              command = self'.packages.centaurid-init;
              depends_on."centauri".condition = "process_healthy";
              log_location = "/tmp/composable-devnet/centauri-init.log";
            };
            osmosis = {
              command = self'.packages.osmosisd-gen;
              readiness_probe.http_get = {
                host = "127.0.0.1";
                port = 36657;
              };
              log_location = "/tmp/composable-devnet/osmosis.log";
            };
            osmosis-init = {
              command = self'.packages.osmosisd-init;
              depends_on."osmosis".condition = "process_healthy";
              log_location = "/tmp/composable-devnet/osmosis-init.log";
              availability = { restart = "on_failure"; };
            };
            picasso = {
              command = self'.packages.zombienet-rococo-local-picasso-dev;
              availability = { restart = "on_failure"; };
              log_location = "/tmp/composable-devnet/zombienet.log";
            };

            hyperspace-client = {
              command = ''
                sleep 20
                COMPOSABLE_DATA=/tmp/composable-devnet/
                HYPERSPACE_DATA="$COMPOSABLE_DATA/hyperspace"
                RUST_LOG="hyperspace=trace,hyperspace_parachain=trace,hyperspace_cosmos=trace"
                export RUST_LOG
                mkdir --parents "$HYPERSPACE_DATA"

                cp --dereference --no-preserve=mode,ownership --force ${self'.packages.hyperspace-config-chain-2} $HYPERSPACE_DATA/config-chain-2.toml  
                cp --dereference --no-preserve=mode,ownership --force ${self'.packages.hyperspace-config-chain-3} $HYPERSPACE_DATA/config-chain-3.toml  
                cp --dereference --no-preserve=mode,ownership --force ${self'.packages.hyperspace-config-core} $HYPERSPACE_DATA/config-core.toml                
                CODE_ID=$(cat $COMPOSABLE_DATA/centauri-devnet/code_id)
                sed -i "s/wasm_code_id = \"0000000000000000000000000000000000000000000000000000000000000000\"/wasm_code_id = \"$CODE_ID\"/" "$HYPERSPACE_DATA/config-chain-2.toml"
                ${self'.packages.hyperspace-composable-rococo-picasso-rococo}/bin/hyperspace create-clients --config-a $HYPERSPACE_DATA/config-chain-3.toml --config-b $HYPERSPACE_DATA/config-chain-2.toml --config-core $HYPERSPACE_DATA/config-core.toml --delay-period 10
              '';
              log_location = "/tmp/composable-devnet/hyperspace-clients.log";
              depends_on = {
                "centauri-init".condition = "process_completed_successfully";
                "centauri".condition = "process_healthy";
              };
              availability = { restart = "on_failure"; };
            };
            hyperspace-connection = {
              command = ''
                COMPOSABLE_DATA=/tmp/composable-devnet/
                HYPERSPACE_DATA="$COMPOSABLE_DATA/hyperspace"
                RUST_LOG="hyperspace=trace,hyperspace_parachain=trace,hyperspace_cosmos=trace"
                export RUST_LOG      
                ${self'.packages.hyperspace-composable-rococo-picasso-rococo}/bin/hyperspace create-connection --config-a $HYPERSPACE_DATA/config-chain-3.toml --config-b $HYPERSPACE_DATA/config-chain-2.toml --config-core $HYPERSPACE_DATA/config-core.toml --delay-period 10
              '';
              log_location = "/tmp/composable-devnet/hyperspace-connection.log";
              depends_on = {
                "hyperspace-client".condition =
                  "process_completed_successfully";
              };
              availability = { restart = "on_failure"; };
            };
            hyperspace-channels = {
              command = ''
                COMPOSABLE_DATA=/tmp/composable-devnet/
                HYPERSPACE_DATA="$COMPOSABLE_DATA/hyperspace"
                RUST_LOG="hyperspace=trace,hyperspace_parachain=trace,hyperspace_cosmos=trace"
                export RUST_LOG
                ${self'.packages.hyperspace-composable-rococo-picasso-rococo}/bin/hyperspace create-channel --config-a $HYPERSPACE_DATA/config-chain-3.toml --config-b $HYPERSPACE_DATA/config-chain-2.toml --config-core $HYPERSPACE_DATA/config-core.toml --delay-period 10 --port-id transfer --version ics20-1 --order unordered
              '';
              log_location = "/tmp/composable-devnet/hyperspace-channels.log";
              depends_on = {
                "hyperspace-connection".condition =
                  "process_completed_successfully";
              };
              availability = { restart = "on_failure"; };
            };
            hyperspace-relay = {
              command = ''
                COMPOSABLE_DATA=/tmp/composable-devnet/
                HYPERSPACE_DATA="$COMPOSABLE_DATA/hyperspace"
                RUST_LOG="hyperspace=trace,hyperspace_parachain=trace,hyperspace_cosmos=trace"
                export RUST_LOG
                ${self'.packages.hyperspace-composable-rococo-picasso-rococo}/bin/hyperspace relay --config-a $HYPERSPACE_DATA/config-chain-3.toml --config-b $HYPERSPACE_DATA/config-chain-2.toml --config-core $HYPERSPACE_DATA/config-core.toml --delay-period 10
              '';
              log_location = "/tmp/composable-devnet/hyperspace-relay.log";
              depends_on = {
                "hyperspace-channels".condition =
                  "process_completed_successfully";
              };
              availability = { restart = "on_failure"; };
            };
          };
        };
      };
    };
}
