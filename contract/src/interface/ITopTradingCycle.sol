// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC721/IERC721.sol";
import {IRiscZeroVerifier} from "risc0/IRiscZeroVerifier.sol";
import {Steel} from "risc0/steel/Steel.sol";

/**
 * @title ITopTradingCycle
 * @dev Interface for the TopTradingCycle contract - designed to be compatible with existing implementation
 */
interface ITopTradingCycle {
    // Enums
    enum Phase {
        Deposit,
        Rank,
        Trade,
        Withdraw,
        Closed
    }

    // Structs - must match exactly with implementation
    struct Token {
        address collection;
        uint256 tokenId;
    }
    
    struct TokenPreferences {
        address owner;
        bytes32 tokenHash;
        bytes32[] preferences;
    }
    
    struct TokenReallocation {
        bytes32 tokenHash;
        address newOwner;
    }
    
    struct Journal {
        Steel.Commitment commitment;
        address ttcContract;
        TokenReallocation[] reallocations;
    }

    // Events
    event PhaseChanged(Phase newPhase);

    // Constants and public state variables
    function imageID() external view returns (bytes32);
    function verifier() external view returns (IRiscZeroVerifier);
    function currentPhase() external view returns (Phase);
    function phaseDuration() external view returns (uint256);
    function phaseStartTimestamp() external view returns (uint256);
    function tradeInitiatedAtBlock() external view returns (uint256);
    function tokenOwners(bytes32 tokenHash) external view returns (address);

    // External functions
    function advancePhase() external returns (Phase);
    function getTokenHash(Token memory token) external pure returns (bytes32);
    function depositNFT(Token calldata token) external returns (bytes32);
    function withdrawNFT(bytes32 tokenHash) external;
    function getDepositedTokens() external view returns (Token[] memory);
    function isTokenDeposited(Token calldata token) external view returns (bool);
    function getCurrentOwner(Token calldata token) external view returns (address);
    function setPreferences(bytes32 ownerTokenHash, bytes32[] calldata preferences) external;
    function getPreferences(bytes32 tokenHash) external view returns (bytes32[] memory);
    function getAllTokenPreferences() external view returns (TokenPreferences[] memory);
    function parseJournal(bytes calldata journalData) external pure returns (Journal memory);
    function reallocateTokens(bytes calldata journalData, bytes calldata seal) external;
    function getTokenFromHash(bytes32 tokenHash) external view returns (Token memory tokenData);
}