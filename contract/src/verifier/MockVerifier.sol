// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {IRiscZeroVerifier, Receipt} from "risc0/IRiscZeroVerifier.sol";

contract MockVerifier is IRiscZeroVerifier {
    function verifyIntegrity(Receipt calldata receipt) external view {}
    function verify(bytes calldata seal, bytes32 imageId, bytes32 journalDigest) external view {}
}
