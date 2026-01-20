import { PrivateBondsContract } from "./private-bonds-contract/artifacts/PrivateBonds.js";
import { AztecAddress } from "@aztec/aztec.js/addresses";
import { createAztecNodeClient } from "@aztec/aztec.js/node";
import { getInitialTestAccountsData } from "@aztec/accounts/testing";
import { TestWallet } from "@aztec/test-wallet/server";
import { openTmpStore } from "@aztec/kv-store/lmdb";

async function main() {
  // Connect to local network
  const node = createAztecNodeClient("http://localhost:8080");

  const store = await openTmpStore();

  const wallet = await TestWallet.create(node);

  const [giggleWalletData, aliceWalletData, bobClinicWalletData] =
    await getInitialTestAccountsData();
  const giggleAccount = await wallet.createSchnorrAccount(
    giggleWalletData.secret,
    giggleWalletData.salt,
  );
  const aliceAccount = await wallet.createSchnorrAccount(
    aliceWalletData.secret,
    aliceWalletData.salt,
  );
  const bobClinicAccount = await wallet.createSchnorrAccount(
    bobClinicWalletData.secret,
    bobClinicWalletData.salt,
  );

  const giggleAddress = (await giggleAccount.getAccount()).getAddress();
  const aliceAddress = (await aliceAccount.getAccount()).getAddress();
  const bobClinicAddress = (await bobClinicAccount.getAccount()).getAddress();

  const bobToken = await PrivateBondsContract.deploy(wallet)
    .send({ from: giggleAddress })
    .deployed();

  await bobToken.methods
    .mint_public(aliceAddress, 100n)
    .send({ from: giggleAddress })
    .wait();

  await bobToken.methods
    .transfer_public(bobClinicAddress, 10n)
    .send({ from: aliceAddress })
    .wait();
}

main().catch(console.error);
