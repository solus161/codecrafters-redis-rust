#!/bin/bash
{
  printf "*3\r\n\$6\r\nCONFIG\r\n\$3\r\nGET\r\n\$3\r\ndir\r\n"
  printf "*3\r\n\$6\r\nCONFIG\r\n\$3\r\nGET\r\n\$10\r\ndbfilename\r\n"

} | nc localhost 6379
