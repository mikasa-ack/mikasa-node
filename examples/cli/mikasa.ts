// @ts-nocheck
const { ApiPromise, WsProvider } = require("@polkadot/api");
import { ContractPromise } from "@polkadot/api-contract";

const { Keyring } = require("@polkadot/keyring");
const util_crypto = require("@polkadot/util-crypto");

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
  const gasLimit = 3000n * 1000000n;
  const storageDepositLimit = null;

  const options = {
    gasLimit,
    storageDepositLimit,
  };
  // Query the contract
  const { gasRequired, storageDeposit, result, output } =
    await contract.query.get(options);

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
