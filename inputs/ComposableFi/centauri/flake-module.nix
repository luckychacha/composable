{ self, ... }: {
  perSystem = { config, self', inputs', pkgs, system, crane, subnix
    , systemCommonRust, ... }:
    let
      cargo-lock = builtins.fromTOML (builtins.readFile ../../code/Cargo.lock);
      centauri-runtime-dep = builtins.head
        (builtins.filter (x: x.name == "pallet-ibc") (cargo-lock.package));
      centauri-runtime-commit =
        builtins.elemAt (builtins.split "#" centauri-runtime-dep.source) 2;
      host = "127.0.0.1";
      hyperspace-picasso-kusama-config-base = {
        channel_whitelist = [ ];
        commitment_prefix = "0x6962632f";
        finality_protocol = "Grandpa";
        connection_id = "connection-0";
        key_type = "sr25519";
        name = "picasso_dev";
        para_id = 2087;
        private_key = "//Alice";
        ss58_version = 49;
        type = "picasso_kusama";
      };

      ibc-composable-to-picasso-1-1 = hyperspace-picasso-kusama-config-base // {
        parachain_rpc_url = "ws://${host}:9988";
        relay_chain_rpc_url = "ws://${host}:9944";
        client_id = "10-grandpa-1";
        connection_id = "connection-1";
        private_key = "//Alice";
      };

      ibc-relayer-config-centauri-to-picasso-kusama-0-0 =
        hyperspace-picasso-kusama-config-base // {
          parachain_rpc_url = "ws://${host}:9988";
          relay_chain_rpc_url = "ws://${host}:9944";
          client_id = "10-grandpa-0";
        };

      ibc-relayer-config-picasso-kusama-to-centauri-0-0 = {
        type = "cosmos";
        name = "centauri";
        rpc_url = "http://${host}:26657";
        grpc_url = "http://${host}:9090";
        websocket_url = "ws://${host}:26657/websocket";
        chain_id = "centauri-dev";
        client_id = "07-tendermint-0";
        connection_id = "connection-0";
        account_prefix = "centauri";
        fee_denom = "ppica";
        fee_amount = "100000000";
        gas_limit = 9223372036854775806;
        store_prefix = "ibc";
        max_tx_size = 20000000;
        wasm_code_id =
          "0000000000000000000000000000000000000000000000000000000000000000";
        skip_optional_client_updates = false;
        channel_whitelist = [ ];
        mnemonic =
          "bottom loan skill merry east cradle onion journey palm apology verb edit desert impose absurd oil bubble sweet glove shallow size build burst effort";
      };

      hyperspace-core-config = { prometheus_endpoint = "https://${host}"; };

      ibc-picasso-to-composable-polkadot-0-0 = {
        type = "composable";
        channel_whitelist = [ ];
        client_id = "10-grandpa-0";
        commitment_prefix = "0x6962632f";
        connection_id = "connection-0";
        finality_protocol = "Grandpa";
        key_type = "sr25519";
        name = "composable";
        para_id = 2087;
        parachain_rpc_url = "ws://${host}:29988";
        private_key = "//Alice";
        relay_chain_rpc_url = "ws://${host}:29944";
        ss58_version = 50;
      };

      toDockerImage = package:
        self.inputs.bundlers.bundlers."${system}".toDockerImage package;

      build-wasm = name: src:
        crane.nightly.buildPackage (systemCommonRust.common-attrs // {
          pname = name;
          version = "0.1";
          src = src;
          cargoBuildCommand =
            "cargo build --release --package ${name} --target wasm32-unknown-unknown";
          RUSTFLAGS = "-C link-arg=-s";
        });

      build-optimized-wasm = name: src: file:
        let wasm = build-wasm name src;
        in pkgs.stdenv.mkDerivation {
          name = name;
          phases = [ "installPhase" ];
          nativeBuildInputs =
            [ pkgs.binaryen self'.packages.subwasm pkgs.hexdump ];
          installPhase = ''
            mkdir --parents $out/lib
            wasm-opt ${wasm}/lib/${file}.wasm -o $out/lib/${file}.wasm -Os --strip-dwarf --debuginfo --mvp-features
            gzip --stdout $out/lib/${file}.wasm > $out/lib/${file}.wasm.gz 
            base64 --wrap=0 $out/lib/${file}.wasm.gz > $out/lib/${file}.wasm.gz.txt
          '';
        };

    in {
      packages = rec {
        centauri-src = self.inputs.centauri-src;
        ics10-grandpa-cw-src = self.inputs.centauri-src;

        ics10-grandpa-cw =
          build-optimized-wasm "ics10-grandpa-cw" ics10-grandpa-cw-src
          "ics10_grandpa_cw";

        centauri-codegen = crane.stable.buildPackage (subnix.subenv // rec {
          name = "centauri-codegen";
          pname = "codegen";
          version = "0.1";
          cargoArtifacts = crane.stable.buildDepsOnly (subnix.subenv // {
            src = centauri-src;
            cargoExtraArgs = "--package codegen";
            cargoTestCommand = "";
            version = "0.1";
            pname = "codegen";
          });
          src = centauri-src;
          cargoExtraArgs = "--package codegen";
          cargoTestCommand = "";
          meta = { mainProgram = "codegen"; };
        });
        centauri-hyperspace = crane.stable.buildPackage (subnix.subenv // {
          name = "centauri-hyperspace";
          version = "0.1";
          cargoArtifacts = crane.stable.buildDepsOnly (subnix.subenv // {
            pname = "hyperspace";
            version = "0.1";
            src = centauri-src;
            doCheck = false;
            cargoExtraArgs = "--package hyperspace";
            cargoTestCommand = "";
          });
          src = centauri-src;
          doCheck = false;
          cargoExtraArgs = "--package hyperspace";
          cargoTestCommand = "";
          meta = { mainProgram = "hyperspace"; };
        });

        composable-rococo-picasso-rococo-subxt-hyperspace-patch =
          pkgs.stdenv.mkDerivation rec {
            name = "composable-rococo-picasso-rococo-subxt-hyperspace-patch";
            pname = "${name}";
            buildInputs = [
              self'.packages.composable-rococo-subxt-client
              self'.packages.picasso-rococo-subxt-client
            ];
            src = centauri-src;
            patchPhase = "true";
            installPhase = ''
              mkdir --parents $out
              set +e
              diff --exclude=mod.rs --recursive --unified $src/utils/subxt/generated/src/composable ${self'.packages.composable-rococo-subxt-client}/ > $out/composable_polkadot.patch
              if [[ $? -ne 1 ]] ; then
                echo "Failed diff"              
              fi                
              diff --exclude=mod.rs --recursive --unified $src/utils/subxt/generated/src/picasso_kusama ${self'.packages.picasso-rococo-subxt-client}/ > $out/picasso_kusama.patch            
              if [[ $? -ne 1 ]] ; then
                echo "Failed diff"              
              fi              
              set -e              
            '';
            dontFixup = true;
            dontStrip = true;
          };

        composable-polkadot-picasso-kusama-subxt-hyperspace-patch =
          pkgs.stdenv.mkDerivation rec {
            name = "composable-polkadot-picasso-kusama-subxt-hyperspace-patch";
            pname = "${name}";
            buildInputs = [
              self'.packages.composable-polkadot-subxt-client
              self'.packages.picasso-kusama-subxt-client
            ];
            src = centauri-src;
            patchPhase = "true";
            installPhase = ''
              mkdir --parents $out
              set +e
              diff --exclude=mod.rs --recursive --unified $src/utils/subxt/generated/src/composable ${self'.packages.composable-polkadot-subxt-client}/ > $out/composable_polkadot.patch
              if [[ $? -ne 1 ]] ; then
                echo "Failed diff"              
              fi                
              diff --exclude=mod.rs --recursive --unified $src/utils/subxt/generated/src/picasso_kusama ${self'.packages.picasso-kusama-subxt-client}/ > $out/picasso_kusama.patch            
              if [[ $? -ne 1 ]] ; then
                echo "Failed diff"              
              fi              
              set -e              
            '';
            dontFixup = true;
            dontStrip = true;
          };

        composable-rococo-picasso-rococo-centauri-patched-src =
          pkgs.stdenv.mkDerivation rec {
            name = "composable-rococo-picasso-rococo-centauri-patched-src";
            pname = "${name}";
            src = centauri-src;
            buildInputs = with pkgs; [ sd git ];
            patchFlags = "--strip=4";
            installPhase = ''
              mkdir --parents $out
              cp --recursive --no-preserve=mode,ownership $src/. $out/

              cd $out/utils/subxt/generated/src/picasso_kusama
              patch ${patchFlags} -- < "${composable-rococo-picasso-rococo-subxt-hyperspace-patch}/picasso_kusama.patch"

              cd $out/utils/subxt/generated/src/composable
              patch ${patchFlags} -- < "${composable-rococo-picasso-rococo-subxt-hyperspace-patch}/composable_polkadot.patch"
              sd "rococo" "polkadot" "$out/utils/subxt/generated/src/composable/relaychain.rs"
            '';
            dontFixup = true;
            dontStrip = true;
          };

        composable-polkadot-picasso-kusama-centauri-patched-src =
          pkgs.stdenv.mkDerivation rec {
            name = "composable-polkadot-picasso-kusama-centauri-patched-src";
            pname = "${name}";
            src = centauri-src;
            buildInputs = with pkgs; [ sd git ];
            patchFlags = "--strip=4";
            installPhase = ''
              mkdir --parents $out
              cp --recursive --no-preserve=mode,ownership $src/. $out/

              cd $out/utils/subxt/generated/src/picasso_kusama
              patch ${patchFlags} -- < "${composable-polkadot-picasso-kusama-subxt-hyperspace-patch}/picasso_kusama.patch"

              cd $out/utils/subxt/generated/src/composable
              patch ${patchFlags} -- < "${composable-polkadot-picasso-kusama-subxt-hyperspace-patch}/composable_polkadot.patch"
              sd "rococo" "polkadot" "$out/utils/subxt/generated/src/composable/relaychain.rs"
            '';
            dontFixup = true;
            dontStrip = true;
          };

        ibc-composable-to-picasso-config-1-1 =
          pkgs.writeText "config-chain-a.toml"
          (self.inputs.nix-std.lib.serde.toTOML ibc-composable-to-picasso-1-1);

        ibc-picasso-to-composable-polkadot-config-0-0 =
          pkgs.writeText "config-chain-b.toml"
          (self.inputs.nix-std.lib.serde.toTOML
            ibc-picasso-to-composable-polkadot-0-0);

        ibc-relayer-config-picasso-kusama-to-centauri-0-0-config =
          pkgs.writeText "config-chain-2.toml"
          (self.inputs.nix-std.lib.serde.toTOML
            ibc-relayer-config-picasso-kusama-to-centauri-0-0);

        ibc-relayer-config-centauri-to-picasso-kusama-0-0-config =
          pkgs.writeText "config-chain-3.toml"
          (self.inputs.nix-std.lib.serde.toTOML
            ibc-relayer-config-centauri-to-picasso-kusama-0-0);

        hyperspace-config-core = pkgs.writeText "config-core.toml"
          (self.inputs.nix-std.lib.serde.toTOML hyperspace-core-config);

        hyperspace-composable-rococo-picasso-rococo = crane.stable.buildPackage
          (subnix.subenv // rec {
            name = "hyperspace-composable-rococo-picasso-rococo";
            pname = name;
            version = "0.1";
            cargoArtifacts = crane.stable.buildDepsOnly (subnix.subenv // {
              src = composable-rococo-picasso-rococo-centauri-patched-src;
              pname = "hyperspace";
              version = "0.1";
              doCheck = false;
              cargoExtraArgs = "--package hyperspace";
              cargoTestCommand = "";
            });
            src = composable-rococo-picasso-rococo-centauri-patched-src;
            doCheck = false;
            cargoExtraArgs = "--package hyperspace";
            cargoTestCommand = "";
            meta = { mainProgram = "hyperspace"; };
          });

        hyperspace-composable-polkadot-picasso-kusama =
          crane.stable.buildPackage (subnix.subenv // rec {
            name = "hyperspace-composable-polkadot-picasso-kusama";
            pname = name;
            version = "0.1";
            cargoArtifacts = crane.stable.buildDepsOnly (subnix.subenv // {
              pname = "hyperspace";
              version = "0.1";
              src = composable-polkadot-picasso-kusama-centauri-patched-src;
              doCheck = false;
              cargoExtraArgs = "--package hyperspace";
              cargoTestCommand = "";
            });
            src = composable-polkadot-picasso-kusama-centauri-patched-src;
            doCheck = false;
            cargoExtraArgs = "--package hyperspace";
            cargoTestCommand = "";
            meta = { mainProgram = "hyperspace"; };
          });

        hyperspace-composable-rococo-picasso-rococo-image =
          toDockerImage hyperspace-composable-rococo-picasso-rococo;

        hyperspace-composable-polkadot-picasso-kusama-image =
          toDockerImage hyperspace-composable-polkadot-picasso-kusama;
      };
    };
}
