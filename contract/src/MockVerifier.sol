// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {RiscZeroMockVerifier} from "risc0/test/RiscZeroMockVerifier.sol";

contract MockVerifier is RiscZeroMockVerifier {
    constructor() RiscZeroMockVerifier(bytes4(0)) {}
}
