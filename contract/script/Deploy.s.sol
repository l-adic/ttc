// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {TestNFT} from "../src/TestNFT.sol";
import {TopTradingCycle} from "../src/TopTradingCycle.sol";

contract DeployScript is Script {
    TestNFT public nft;
    TopTradingCycle public ttc;

    function setUp() public {}

    function run() public {
        // Start broadcasting transactions
        vm.startBroadcast();

        // Deploy the NFT contract
        nft = new TestNFT();
        console.log("NFT deployed at:", address(nft));

        // Deploy TTC contract with NFT address
        ttc = new TopTradingCycle(address(nft));
        console.log("TTC deployed at:", address(ttc));

        // Optional: Mint some initial NFTs for testing
        // nft.safeMint(msg.sender, 1);
        // nft.safeMint(msg.sender, 2);

        vm.stopBroadcast();
    }
}