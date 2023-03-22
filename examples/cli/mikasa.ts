// @ts-nocheck
const { ApiPromise, WsProvider } = require("@polkadot/api");
import { ContractPromise } from "@polkadot/api-contract";
import type { WeightV2 } from "@polkadot/types/interfaces";
import { BN, BN_ONE } from "@polkadot/util";

const { Keyring } = require("@polkadot/keyring");
const util_crypto = require("@polkadot/util-crypto");
const MAX_CALL_WEIGHT = new BN(5_000_000_000_000).isub(BN_ONE);
const PROOFSIZE = new BN(1_000_000);
main().catch((error) => {
  console.error(error);
  process.exit(-1);
});

async function main() {
  const wsProvider = new WsProvider("ws://127.0.0.1:9944");
  const api = await ApiPromise.create({ provider: wsProvider });

  await getChainInfo(api);

  // Construct the keyring after the API (crypto has an async init)
  const keyring = new Keyring({ type: "sr25519" });

  // Add Alice to our keyring with a hard-derivation path (empty phrase, so uses dev)
  const alice = keyring.addFromUri("//Alice");

  // Read metadata json file
  const metadata = require("./mikasa.json");
  const address = "5CA1V4NBFkiNfprx5bcTauNZemnseJCb7hntyzMDVzZ1xsVb";
  const contract = new ContractPromise(api, metadata, address);
  const storageDepositLimit = null;
  const gasLimit = api?.registry.createType("WeightV2", {
    refTime: MAX_CALL_WEIGHT,
    proofSize: PROOFSIZE,
  }) as WeightV2;

  const options = {
    gasLimit,
    storageDepositLimit,
  };
  // Query the contract
  const { gasRequired, storageDeposit, result, output } =
    await contract.query.get(alice.address, options);

  // The actual result from RPC as `ContractExecResult`
  console.log(result.toHuman());
}

async function getChainInfo(api) {
  // Retrieve the chain & node information information via rpc calls
  const [chain, nodeName, nodeVersion, genesisHash] = await Promise.all([
    api.rpc.system.chain(),
    api.rpc.system.name(),
    api.rpc.system.version(),
    api.genesisHash.toHex(),
  ]);
  console.log(
    `Connected to chain ${chain} using ${nodeName} v${nodeVersion} with genesis hash ${genesisHash}`
  );
}
