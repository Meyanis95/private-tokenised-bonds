// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script} from "forge-std/Script.sol";
import {PrivateBond} from "../src/PrivateBond.sol";
import {HonkVerifier} from "../src/Verifier.sol";

contract PrivateBondScript is Script {
    HonkVerifier public verifier;
    PrivateBond public privateBond;
    address public owner;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        owner = address(this);
        verifier = new HonkVerifier();
        
        privateBond = new PrivateBond(address(verifier), owner);

        vm.stopBroadcast();
        
        require(address(verifier) != address(0), "Verifier deployment failed");
        require(address(privateBond) != address(0), "PrivateBond deployment failed");
    }
}
