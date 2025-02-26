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
 * @dev Contract for managing NFT custody and transfers in a top trading cycle
 */
contract TopTradingCycle is ERC721Holder, Ownable, ReentrancyGuard {

    // The ERC721 contract this TTC operates on
    IERC721 public immutable nftContract;
    
    // Mapping from token ID to current owner
    mapping(uint256 => address) public tokenOwners;
    
    // Array to keep track of all deposited token IDs
    uint256[] private depositedTokens;
    
    // Mapping to track index of token ID in depositedTokens array
    mapping(uint256 => uint256) private tokenIdToIndex;
    
    // Event for ownership transfers within the contract (not actual NFT transfers)
    event InternalOwnershipTransferred(address indexed from, address indexed to, uint256 indexed tokenId);
    
    // Event for when preferences are updated
    event PreferencesUpdated(uint256 indexed tokenId, uint256[] preferences);

    // Mapping from token ID to its preference list
    mapping(uint256 => uint256[]) public tokenPreferences;

    // Struct to represent a token and its preferences
    struct TokenPreferences {
        address owner;
        uint256 tokenId;
        uint256[] preferences;
    }

    /**
     * @dev Constructor sets the NFT contract address
     * @param _nftContract Address of the ERC721 contract
     */
    constructor(address _nftContract) Ownable(msg.sender) {
        require(_nftContract != address(0), "Invalid NFT contract address");
        nftContract = IERC721(_nftContract);
    }

    /**
     * @dev Allows a user to deposit their NFT into the contract
     * @param tokenId The ID of the token to deposit
     */
    function depositNFT(uint256 tokenId) external nonReentrant {
        require(nftContract.ownerOf(tokenId) == msg.sender, "Not token owner");
        require(tokenOwners[tokenId] == address(0), "Token already deposited");
        
        // Transfer the NFT to this contract
        nftContract.safeTransferFrom(msg.sender, address(this), tokenId);
        
        // Record the depositor as the owner in our contract
        tokenOwners[tokenId] = msg.sender;
        
        // Add token to tracking array
        tokenIdToIndex[tokenId] = depositedTokens.length;
        depositedTokens.push(tokenId);
    }

    /**
     * @dev Allows the current owner to withdraw their NFT
     * @param tokenId The ID of the token to withdraw
     */
    function withdrawNFT(uint256 tokenId) external nonReentrant {
        require(tokenOwners[tokenId] == msg.sender, "Not token owner");
        
        // Clear ownership record
        delete tokenOwners[tokenId];
        
        // Transfer the NFT back to the owner
        nftContract.safeTransferFrom(address(this), msg.sender, tokenId);
    }

    /**
     * @dev Reset contract state by clearing all tracking data and preferences
     */
    function cleanup() external {
        // Get current tokens for iteration
        uint256[] memory tokens = depositedTokens;
        
        // Clear all contract state
        for (uint256 i = 0; i < tokens.length; i++) {
            delete tokenIdToIndex[tokens[i]];
            delete tokenPreferences[tokens[i]];
        }
        
        // Clear the array
        delete depositedTokens;
    }

    /**
     * @dev Internal function to transfer NFT ownership within the contract
     * @param from Current owner address
     * @param to New owner address
     * @param tokenId The ID of the token to transfer
     */
    function _transferNFTOwnership(address from, address to, uint256 tokenId) internal {
        require(tokenOwners[tokenId] == from, "Not token owner");
        require(to != address(0), "Invalid recipient");
        
        tokenOwners[tokenId] = to;
        
        emit InternalOwnershipTransferred(from, to, tokenId);
    }

    /**
     * @dev View function to get all deposited token IDs
     * @return Array of all token IDs currently deposited in the contract
     */
    function getDepositedTokens() external view returns (uint256[] memory) {
        return depositedTokens;
    }

    /**
     * @dev View function to check if a token is deposited in the contract
     * @param tokenId The ID of the token to check
     * @return bool indicating if the token is deposited
     */
    function isTokenDeposited(uint256 tokenId) external view returns (bool) {
        return tokenOwners[tokenId] != address(0);
    }

    /**
     * @dev View function to get the current owner of a deposited token
     * @param tokenId The ID of the token to check
     * @return address of the current owner
     */
    function getCurrentOwner(uint256 tokenId) external view returns (address) {
        return tokenOwners[tokenId];
    }

    /**
     * @dev Allows a token owner to set their preferences for trades
     * @param tokenId The ID of the token whose preferences are being set
     * @param preferences Array of token IDs representing trade preferences in order of preference
     */
    function setPreferences(uint256 tokenId, uint256[] calldata preferences) external {
        require(tokenOwners[tokenId] == msg.sender, "Not token owner");
        
        // Validate all preference tokens exist in the contract
        for (uint256 i = 0; i < preferences.length; i++) {
            require(tokenOwners[preferences[i]] != address(0), "Invalid preference token");
        }
        
        // Clear existing preferences and set new ones
        delete tokenPreferences[tokenId];
        tokenPreferences[tokenId] = preferences;
        
        emit PreferencesUpdated(tokenId, preferences);
    }

    /**
     * @dev View function to get the preferences for a specific token
     * @param tokenId The ID of the token to check preferences for
     * @return Array of token IDs representing trade preferences
     */
    function getPreferences(uint256 tokenId) external view returns (uint256[] memory) {
        return tokenPreferences[tokenId];
    }

    /**
     * @dev View function to get all tokens and their preferences
     * @return Array of TokenPreferences structs containing each token and its preference list
     */
    function getAllTokenPreferences() external view returns (TokenPreferences[] memory) {
        uint256 totalTokens = depositedTokens.length;
        TokenPreferences[] memory allPreferences = new TokenPreferences[](totalTokens);
        
        for (uint256 i = 0; i < totalTokens; i++) {
            uint256 tokenId = depositedTokens[i];
            allPreferences[i] = TokenPreferences({
                owner: tokenOwners[tokenId],
                tokenId: tokenId,
                preferences: tokenPreferences[tokenId]
            });
        }
        
        return allPreferences;
    }

    // Struct to represent a token reallocation pair
    struct TokenReallocation {
        uint256 tokenId;
        address newOwner;
    }

    struct Journal {
        Steel.Commitment commitment ;
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
     * @dev STUB FUNCTION - Atomic reallocation of token ownership
     * This function will implement the core top trading cycle algorithm
     * by reallocating tokens according to the computed cycles.
     * For each (tokenId, newOwner) pair, newOwner becomes the owner of tokenId.
     * 
     * Requirements:
     * - All tokens must exist in the contract
     * - Each token can only appear once
     * - The reallocation must form valid trading cycles
     * 
     * @param journalData bytes representing the abi encoded journal
     */

    function reallocateTokens(bytes calldata journalData) external {

         // Decode and validate the journal data
        Journal memory journal = parseJournal(journalData);
        require(journal.ttcContract == address(this), "Invalid token address");
        // require(Steel.validateCommitment(journal.commitment), "Invalid commitment");

        // Verify the proof
        // bytes32 journalHash = sha256(journalData);
        // verifier.verify(seal, imageID, journalHash);

        for (uint256 i = 0; i < journal.reallocations.length; i++) {
            TokenReallocation memory realloc = journal.reallocations[i];
            address currentOwner = tokenOwners[realloc.tokenId];
            _transferNFTOwnership(currentOwner, realloc.newOwner, realloc.tokenId);
        }
    }
}
