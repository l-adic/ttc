// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC721/IERC721.sol";
import "@openzeppelin/contracts/token/ERC721/utils/ERC721Holder.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IRiscZeroVerifier} from "risc0/IRiscZeroVerifier.sol";
import {Steel, Encoding} from "risc0/steel/Steel.sol";
import {ImageID} from "./ImageID.sol";
import "./interface/ITopTradingCycle.sol";


/**
 * @title TopTradingCycle
 * @dev Contract for managing NFTs from any collection in a top trading cycle
 */
contract TopTradingCycle is ITopTradingCycle, ERC721Holder, Ownable, ReentrancyGuard {
    bytes32 public constant imageID = ImageID.PROVABLE_TTC_ID;

    IRiscZeroVerifier public immutable verifier;
    
    Phase public currentPhase;
    uint256 public phaseDuration;
    uint256 public phaseStartTimestamp;
    uint256 public tradeInitiatedAtBlock;

    // Mapping from token hash to current owner
    mapping(bytes32 => address) public tokenOwners;
    
    // Array to keep track of all deposited tokens
    Token[] private depositedTokens;
    
    // Mapping to track index of token in depositedTokens array
    mapping(bytes32 => uint256) private tokenHashToIndex;

    // Mapping from token hash to its preference list (which is a list of token hashes)
    mapping(bytes32 => bytes32[]) public tokenPreferences;

    /**
     * @dev Constructor sets the verifier contract address and phase duration
     * @param _verifier Address of the Verifier contract
     * @param _phaseDuration Duration of each phase in seconds
     */
    constructor(IRiscZeroVerifier _verifier, uint256 _phaseDuration) Ownable(msg.sender) {
        require(address(_verifier) != address(0), "Invalid Verifier address");
        verifier = _verifier;
        phaseDuration = _phaseDuration;
        currentPhase = Phase.Deposit;
        phaseStartTimestamp = block.timestamp;
    }

    /**
     * @dev Modifier to check if the current phase matches the required phase
     * @param requiredPhase The phase in which the function is allowed to execute
     */
    modifier onlyInPhase(Phase requiredPhase) {
        require(currentPhase == requiredPhase, "Not in the correct phase");
        _;
    }

    /**
     * @dev Advances to the next phase
     * Duration check is only applied for Deposit->Rank and Rank->Trade transitions
     * For Withdraw->Closed, all NFTs must be withdrawn
     * @return The new current phase
     */
    function advancePhase() external returns (Phase) {
        if (currentPhase == Phase.Deposit) {
            // Check duration for Deposit -> Rank transition
            require(block.timestamp >= phaseStartTimestamp + phaseDuration, "Deposit phase duration not yet passed");
            currentPhase = Phase.Rank;
        } else if (currentPhase == Phase.Rank) {
            // Check duration for Rank -> Trade transition
            require(block.timestamp >= phaseStartTimestamp + phaseDuration, "Rank phase duration not yet passed");
            tradeInitiatedAtBlock = block.number;
            currentPhase = Phase.Trade;
        } else if (currentPhase == Phase.Trade) {
            // No duration check for Trade -> Withdraw
            require(block.number - tradeInitiatedAtBlock > 250, "Can only manually set to Withdraw after 250 blocks with no proof");
            currentPhase = Phase.Withdraw;
        } else if (currentPhase == Phase.Withdraw) {
            // Check that all NFTs have been withdrawn
            require(depositedTokens.length == 0, "Not all NFTs have been withdrawn");
            payable(owner()).transfer(address(this).balance);
            currentPhase = Phase.Closed;
        }

        phaseStartTimestamp = block.timestamp;
        emit PhaseChanged(currentPhase);

        return currentPhase;
    }

    /**
     * @dev Generate a unique hash for a token
     * @param token The Token struct containing collection address and tokenId
     * @return The hash representing this token
     */
    function getTokenHash(Token memory token) public pure returns (bytes32) {
        return keccak256(abi.encodePacked(token.collection, token.tokenId));
    }

    /**
     * @dev Allows a user to deposit their NFT into the contract
     * @param token The Token struct containing collection address and tokenId
     * @return The token hash for the deposited token
     */
    function depositNFT(Token calldata token) external nonReentrant onlyInPhase(Phase.Deposit) returns (bytes32) {
        IERC721 nftContract = IERC721(token.collection);
        
        bytes32 tokenHash = getTokenHash(token);
        require(tokenOwners[tokenHash] == address(0), "Token already deposited");
        
        // Transfer the NFT to this contract
        nftContract.safeTransferFrom(msg.sender, address(this), token.tokenId);
        
        // Record the depositor as the owner in our contract
        tokenOwners[tokenHash] = msg.sender;
        
        // Add token to tracking array
        tokenHashToIndex[tokenHash] = depositedTokens.length;
        depositedTokens.push(token);
        
        return tokenHash;
    }

    /**
     * @dev Allows the current owner to withdraw their NFT
     * @param tokenHash The token hash
     */
    function withdrawNFT(bytes32 tokenHash) external nonReentrant onlyInPhase(Phase.Withdraw) {
        require(tokenOwners[tokenHash] == msg.sender, "Not token owner");
        
        // Get token data
        uint256 tokenIndex = tokenHashToIndex[tokenHash];
        Token memory tokenData = depositedTokens[tokenIndex];
        
        // Clear all token data
        delete tokenOwners[tokenHash];
        delete tokenPreferences[tokenHash];
        
        // Remove token from the depositedTokens array using the "swap and pop" pattern
        uint256 lastTokenIndex = depositedTokens.length - 1;
        
        // If the token to remove is not the last one, move the last token to its position
        if (tokenIndex != lastTokenIndex) {
            // Get the last token in the array
            Token memory lastToken = depositedTokens[lastTokenIndex];
            
            // Move the last token to the position of the token being removed
            depositedTokens[tokenIndex] = lastToken;
            
            // Update the index mapping for the moved token
            bytes32 lastTokenHash = getTokenHash(lastToken);
            tokenHashToIndex[lastTokenHash] = tokenIndex;
        }
        
        // Remove the last element (which is either the token we want to remove or a duplicate)
        depositedTokens.pop();
        
        // Delete the token's index mapping entry
        delete tokenHashToIndex[tokenHash];
        
        // Transfer the NFT back to the owner
        IERC721(tokenData.collection).safeTransferFrom(address(this), msg.sender, tokenData.tokenId);
    }

    /**
     * @dev Internal function to transfer NFT ownership within the contract
     * @param from Current owner address
     * @param to New owner address
     * @param tokenHash The token hash
     */
    function _transferNFTOwnership(address from, address to, bytes32 tokenHash) internal {
        require(tokenOwners[tokenHash] == from, "Not token owner");
        require(to != address(0), "Invalid recipient");
        
        tokenOwners[tokenHash] = to;
    }

    /**
     * @dev View function to get all deposited tokens
     * @return Array of all tokens currently deposited in the contract
     */
    function getDepositedTokens() external view returns (Token[] memory) {
        return depositedTokens;
    }

    /**
     * @dev View function to check if a token is deposited in the contract
     * @param token The Token struct containing collection address and tokenId
     * @return bool indicating if the token is deposited
     */
    function isTokenDeposited(Token calldata token) external view returns (bool) {
        bytes32 tokenHash = getTokenHash(token);
        return tokenOwners[tokenHash] != address(0);
    }

    /**
     * @dev View function to get the current owner of a deposited token
     * @param token The Token struct containing collection address and tokenId
     * @return address of the current owner
     */
    function getCurrentOwner(Token calldata token) external view returns (address) {
        bytes32 tokenHash = getTokenHash(token);
        return tokenOwners[tokenHash];
    }

    /**
     * @dev Allows a token owner to set their preferences for trades
     * @param ownerTokenHash The token hash of the owner's token
     * @param preferences Array of token hashes representing preferences
     */
    function setPreferences(
        bytes32 ownerTokenHash,
        bytes32[] calldata preferences
    ) external onlyInPhase(Phase.Rank) {
        require(tokenOwners[ownerTokenHash] == msg.sender, "Not token owner");
        
        // Validate all preference tokens exist in the contract
        for (uint256 i = 0; i < preferences.length; i++) {
            require(tokenOwners[preferences[i]] != address(0), "Invalid preference token");
        }
        
        // Clear existing preferences and set new ones
        delete tokenPreferences[ownerTokenHash];
        tokenPreferences[ownerTokenHash] = preferences;
    }

    /**
     * @dev View function to get the preferences for a specific token
     * @param tokenHash The Token hash representing the collection address and tokenId
     * @return Array of token hashes representing trade preferences
     */
    function getPreferences(bytes32 tokenHash) external view returns (bytes32[] memory) {
        return tokenPreferences[tokenHash];
    }

    /**
     * @dev View function to get all tokens and their preferences
     * @return Array of TokenPreferences structs containing each token and its preference list
     */
    function getAllTokenPreferences() external view returns (TokenPreferences[] memory) {
        uint256 totalTokens = depositedTokens.length;
        TokenPreferences[] memory allPreferences = new TokenPreferences[](totalTokens);
        
        for (uint256 i = 0; i < totalTokens; i++) {
            Token memory tokenData = depositedTokens[i];
            bytes32 tokenHash = getTokenHash(tokenData);
            
            allPreferences[i] = TokenPreferences({
                owner: tokenOwners[tokenHash],
                tokenHash: tokenHash,
                preferences: tokenPreferences[tokenHash]
            });
        }
        
        return allPreferences;
    }

    /**
     * @dev Parse journal data from bytes into a Journal struct
     * @param journalData The ABI encoded journal data
     * @return journal The decoded Journal struct
     */
    function parseJournal(bytes calldata journalData) public pure returns (Journal memory) {
        Journal memory journal = abi.decode(journalData, (Journal));
        return journal;
    }

    /**
     * @dev Reallocate token ownership according to the computed trading cycles
     * For each (collection, tokenId, newOwner) triplet, newOwner becomes the owner of the token.
     * 
     * Requirements:
     * - All tokens must exist in the contract
     * - Each token can only appear once
     * - The reallocation must form valid trading cycles
     * 
     * @param journalData bytes representing the abi encoded journal
     * @param seal The verification seal from RISC Zero
     */
    function reallocateTokens(bytes calldata journalData, bytes calldata seal) external onlyInPhase(Phase.Trade) {
        // Decode and validate the journal data
        Journal memory journal = parseJournal(journalData);
        require(journal.ttcContract == address(this), "Invalid contract address");
        (uint240 claimID,) = Encoding.decodeVersionedID(journal.commitment.id);
        require(claimID == tradeInitiatedAtBlock, "Commitment doesn't represent state at trade block number");
        require(Steel.validateCommitment(journal.commitment), "Invalid commitment");

        // Verify the proof
        bytes32 journalHash = sha256(journalData);
        verifier.verify(seal, imageID, journalHash);

        for (uint256 i = 0; i < journal.reallocations.length; i++) {
            TokenReallocation memory realloc = journal.reallocations[i];
            bytes32 tokenHash = realloc.tokenHash;
            address currentOwner = tokenOwners[tokenHash];
            
            _transferNFTOwnership(currentOwner, realloc.newOwner, tokenHash);
        }
        currentPhase = Phase.Withdraw;
    }

    function getTokenFromHash(bytes32 tokenHash) external view returns (Token memory tokenData) {
        require(tokenOwners[tokenHash] != address(0), "Token hash not found");
        uint256 index = tokenHashToIndex[tokenHash];
        require(index < depositedTokens.length, "Token index out of bounds");
        
        return depositedTokens[index];
    }

}
