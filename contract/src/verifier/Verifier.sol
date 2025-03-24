// SPDX-License-Identifier: MIT
pragma solidity ^0.8.13;

import {ControlID, RiscZeroGroth16Verifier} from "risc0/groth16/RiscZeroGroth16Verifier.sol";


contract Verifier is RiscZeroGroth16Verifier {
    constructor() RiscZeroGroth16Verifier(ControlID.CONTROL_ROOT, ControlID.BN254_CONTROL_ID) {}
}
