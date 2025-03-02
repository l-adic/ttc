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
    mapping(bytes32 => uint256) private tokenToIndex;
    
    // Event for ownership transfers within the contract (not actual NFT transfers)
    event InternalOwnershipTransferred(address indexed from, address indexed to, address indexed collection, uint256 tokenId);
    
    // Event for when preferences are updated
    event PreferencesUpdated(address indexed collection, uint256 indexed tokenId, bytes32[] preferences);

    // Mapping from token hash to its preference list (which is a list of token hashes)
    mapping(bytes32 => bytes32[]) public tokenPreferences;

    // Struct to represent a token and its preferences
    struct TokenPreferences {
        address owner;
        bytes32 token;
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
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token
     * @return The hash representing this token
     */
    function getTokenHash(address collection, uint256 tokenId) public pure returns (bytes32) {
        return keccak256(abi.encodePacked(collection, tokenId));
    }

    /**
     * @dev Allows a user to deposit their NFT into the contract
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token to deposit
     */
    function depositNFT(address collection, uint256 tokenId) external nonReentrant {
        require(collection != address(0), "Invalid collection address");
        IERC721 nftContract = IERC721(collection);
        require(nftContract.ownerOf(tokenId) == msg.sender, "Not token owner");
        
        bytes32 token = getTokenHash(collection, tokenId);
        require(tokenOwners[token] == address(0), "Token already deposited");
        
        // Transfer the NFT to this contract
        nftContract.safeTransferFrom(msg.sender, address(this), tokenId);
        
        // Record the depositor as the owner in our contract
        tokenOwners[token] = msg.sender;
        
        // Add token to tracking array
        Token memory newToken = Token({
            collection: collection,
            tokenId: tokenId
        });
        
        tokenToIndex[token] = depositedTokens.length;
        depositedTokens.push(newToken);
    }

    /**
     * @dev Allows the current owner to withdraw their NFT
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token to withdraw
     */
    function withdrawNFT(address collection, uint256 tokenId) external nonReentrant {
        bytes32 token = getTokenHash(collection, tokenId);
        require(tokenOwners[token] == msg.sender, "Not token owner");
        
        // Clear all token data
        delete tokenOwners[token];
        delete tokenPreferences[token];
        
        // Remove token from the depositedTokens array using the "swap and pop" pattern
        uint256 tokenIndex = tokenToIndex[token];
        uint256 lastTokenIndex = depositedTokens.length - 1;
        
        // If the token to remove is not the last one, move the last token to its position
        if (tokenIndex != lastTokenIndex) {
            // Get the last token in the array
            Token memory lastToken = depositedTokens[lastTokenIndex];
            
            // Move the last token to the position of the token being removed
            depositedTokens[tokenIndex] = lastToken;
            
            // Update the index mapping for the moved token
            bytes32 lastTokenHash = getTokenHash(lastToken.collection, lastToken.tokenId);
            tokenToIndex[lastTokenHash] = tokenIndex;
        }
        
        // Remove the last element (which is either the token we want to remove or a duplicate)
        depositedTokens.pop();
        
        // Delete the token's index mapping entry
        delete tokenToIndex[token];
        
        // Transfer the NFT back to the owner
        IERC721(collection).safeTransferFrom(address(this), msg.sender, tokenId);
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
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token to transfer
     */
    function _transferNFTOwnership(address from, address to, address collection, uint256 tokenId) internal {
        bytes32 token = getTokenHash(collection, tokenId);
        require(tokenOwners[token] == from, "Not token owner");
        require(to != address(0), "Invalid recipient");
        
        tokenOwners[token] = to;
        
        emit InternalOwnershipTransferred(from, to, collection, tokenId);
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
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token to check
     * @return bool indicating if the token is deposited
     */
    function isTokenDeposited(address collection, uint256 tokenId) external view returns (bool) {
        bytes32 token = getTokenHash(collection, tokenId);
        return tokenOwners[token] != address(0);
    }

    /**
     * @dev View function to get the current owner of a deposited token
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token to check
     * @return address of the current owner
     */
    function getCurrentOwner(address collection, uint256 tokenId) external view returns (address) {
        bytes32 token = getTokenHash(collection, tokenId);
        return tokenOwners[token];
    }

    /**
     * @dev Allows a token owner to set their preferences for trades
     * @param ownerCollection The ERC721 contract address of the owner's token
     * @param ownerTokenId The ID of the owner's token
     * @param preferenceCollections Array of ERC721 contract addresses for preferred tokens
     * @param preferenceTokenIds Array of token IDs for preferred tokens
     */
    function setPreferences(
        address ownerCollection, 
        uint256 ownerTokenId, 
        address[] calldata preferenceCollections, 
        uint256[] calldata preferenceTokenIds
    ) external {
        require(preferenceCollections.length == preferenceTokenIds.length, "Collections and tokenIds length mismatch");
        
        bytes32 ownerToken = getTokenHash(ownerCollection, ownerTokenId);
        require(tokenOwners[ownerToken] == msg.sender, "Not token owner");
        
        // Prepare array for preference tokens
        bytes32[] memory preferences = new bytes32[](preferenceCollections.length);
        
        // Validate all preference tokens exist in the contract
        for (uint256 i = 0; i < preferenceCollections.length; i++) {
            bytes32 prefToken = getTokenHash(preferenceCollections[i], preferenceTokenIds[i]);
            require(tokenOwners[prefToken] != address(0), "Invalid preference token");
            preferences[i] = prefToken;
        }
        
        // Clear existing preferences and set new ones
        delete tokenPreferences[ownerToken];
        tokenPreferences[ownerToken] = preferences;
        
        emit PreferencesUpdated(ownerCollection, ownerTokenId, preferences);
    }

    /**
     * @dev View function to get the preferences for a specific token
     * @param collection The ERC721 contract address
     * @param tokenId The ID of the token to check preferences for
     * @return Array of token hashes representing trade preferences
     */
    function getPreferences(address collection, uint256 tokenId) external view returns (bytes32[] memory) {
        bytes32 token = getTokenHash(collection, tokenId);
        return tokenPreferences[token];
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
            bytes32 token = getTokenHash(tokenData.collection, tokenData.tokenId);
            
            allPreferences[i] = TokenPreferences({
                owner: tokenOwners[token],
                token: token,
                preferences: tokenPreferences[token]
            });
        }
        
        return allPreferences;
    }

    // Struct to represent a token reallocation pair
    struct TokenReallocation {
        bytes32 token;
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
            bytes32 token = realloc.token;
            address currentOwner = tokenOwners[token];
            
            // Get the token's collection and ID for the transfer
            uint256 index = tokenToIndex[token];
            require(index < depositedTokens.length, "Token not found");
            Token memory tokenData = depositedTokens[index];
            
            _transferNFTOwnership(currentOwner, realloc.newOwner, tokenData.collection, tokenData.tokenId);
        }
    }

    /**
     * @dev Helper function to get token information from a hash
     * @param token The token hash
     * @return tokenData The Token struct containing collection address and token ID
     */
    function getTokenInfoFromHash(bytes32 token) external view returns (Token memory tokenData) {
        uint256 index = tokenToIndex[token];
        require(index < depositedTokens.length, "Token not found");
        
        return depositedTokens[index];
    }
}