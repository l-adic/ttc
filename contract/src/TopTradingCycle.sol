// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

import "@openzeppelin/contracts/token/ERC721/IERC721.sol";
import "@openzeppelin/contracts/token/ERC721/utils/ERC721Holder.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import {IRiscZeroVerifier} from "risc0/IRiscZeroVerifier.sol";
import {Steel} from "risc0/steel/Steel.sol";
import {ImageID} from "./ImageID.sol";


/**
 * @title TopTradingCycle
 * @dev Contract for managing NFTs from any collection in a top trading cycle
 */
contract TopTradingCycle is ERC721Holder, Ownable, ReentrancyGuard {
    bytes32 public constant imageID = ImageID.PROVABLE_TTC_ID;

    IRiscZeroVerifier public immutable verifier;
    
    // Struct to represent a token from any ERC721 collection
    struct Token {
        address collection;  // The ERC721 contract address
        uint256 tokenId;     // The token ID within that collection
    }
    
    // Mapping from token hash to current owner
    mapping(bytes32 => address) public tokenOwners;
    
    // Array to keep track of all deposited tokens
    Token[] private depositedTokens;
    
    // Mapping to track index of token in depositedTokens array
    mapping(bytes32 => uint256) private tokenHashToIndex;
    
    // Event for ownership transfers within the contract (not actual NFT transfers)
    event InternalOwnershipTransferred(address indexed from, address indexed to, address indexed collection, uint256 tokenId);
    
    // Event for when preferences are updated
    event PreferencesUpdated(address indexed collection, uint256 indexed tokenId, bytes32[] preferences);

    // Mapping from token hash to its preference list (which is a list of token hashes)
    mapping(bytes32 => bytes32[]) public tokenPreferences;

    // Struct to represent a token and its preferences
    struct TokenPreferences {
        address owner;
        bytes32 tokenHash;
        bytes32[] preferences;
    }

    /**
     * @dev Constructor sets the verifier contract address
     * @param _verifier Address of the Verifier contract
     */
    constructor(IRiscZeroVerifier _verifier) Ownable(msg.sender) {
        require(address(_verifier) != address(0), "Invalid Verifier address");
        verifier = _verifier;
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
    function depositNFT(Token calldata token) external nonReentrant returns (bytes32) {
        require(token.collection != address(0), "Invalid collection address");
        IERC721 nftContract = IERC721(token.collection);
        require(nftContract.ownerOf(token.tokenId) == msg.sender, "Not token owner");
        
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
    function withdrawNFT(bytes32 tokenHash) external nonReentrant {
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
     * @dev Reset contract state by clearing all tracking data and preferences
     * This should only be called when no NFTs remain in the contract
     */
    function cleanup() external onlyOwner {
        // Verify no NFTs remain (optional safety check)
        require(depositedTokens.length == 0, "NFTs still in contract");
        
        // Reset depositedTokens to a fresh empty array
        delete depositedTokens;
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
        
        // Get token data for the event
        Token memory tokenData = depositedTokens[tokenHashToIndex[tokenHash]];
        
        emit InternalOwnershipTransferred(from, to, tokenData.collection, tokenData.tokenId);
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
    ) external {
        require(tokenOwners[ownerTokenHash] == msg.sender, "Not token owner");
        
        // Validate all preference tokens exist in the contract
        for (uint256 i = 0; i < preferences.length; i++) {
            require(tokenOwners[preferences[i]] != address(0), "Invalid preference token");
        }
        
        // Clear existing preferences and set new ones
        delete tokenPreferences[ownerTokenHash];
        tokenPreferences[ownerTokenHash] = preferences;
        
        // Get the token details for the event
        Token memory tokenData = depositedTokens[tokenHashToIndex[ownerTokenHash]];
        
        emit PreferencesUpdated(tokenData.collection, tokenData.tokenId, preferences);
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

    // Struct to represent a token reallocation pair
    struct TokenReallocation {
        bytes32 tokenHash;
        address newOwner;
    }

    struct Journal {
        Steel.Commitment commitment;
        address ttcContract;
        TokenReallocation[] reallocations;
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
    function reallocateTokens(bytes calldata journalData, bytes calldata seal) external {
        // Decode and validate the journal data
        Journal memory journal = parseJournal(journalData);
        require(journal.ttcContract == address(this), "Invalid contract address");
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
    }

    /**
     * @dev Helper function to get token information from a hash
     * @dev Helper function to get token information from a hash
     * @param tokenHash The hash of the token
     * @return tokenData The Token struct containing collection address and token ID
     */
    function getTokenFromHash(bytes32 tokenHash) external view returns (Token memory tokenData) {
        uint256 index = tokenHashToIndex[tokenHash];
        require(index < depositedTokens.length, "Token not found");
        
        return depositedTokens[index];
    }
}